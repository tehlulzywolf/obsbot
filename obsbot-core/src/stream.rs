//! UVC video streaming for the Tiny 2 Lite, fully in userspace (`nusb`).
//!
//! Why not /dev/video0: WSL's kernel uvcvideo binds the camera but exposes
//! no XU/v4l-subdev char nodes, and its claim conflicts with our nusb
//! control transport — so video rides the same nusb handle family as
//! control. Descriptor facts (measured via `lsusb -v`):
//!   VS interface 1, streaming endpoint 0x82 **bulk IN**, mps 512.
//!   format 1 = MJPEG: 1920x1080@{60..15}, 3840x2160@{30..15}, 1280x720@{60..15}
//!   format 2 = YUY2 :  640x360@30, 640x480@30
//!
//! Negotiation is UVC 1.0 probe/commit over the VS interface (wValue =
//! selector<<8, wIndex = 1<<8|alt): we GET_CUR probe (selector 1) first and
//! edit the device's own struct copy, so the 34-byte layout never has to be
//! hardcoded from the spec. Then SET_CUR probe, SET_CUR commit with
//! bCommand=1, then read bulk 0x82 and reassemble UVC payload frames.
//!
//! MJPEG payloads are decoded with `zune-jpeg`; YUY2 is converted directly.
//! Frames leave through an `mpsc` channel as `Frame { w, h, rgba }`.

use nusb::{
    transfer::{Bulk, ControlIn, ControlOut, ControlType, In, Recipient},
    Device, MaybeFuture,
};
use std::io::Read as _;
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

const VS_IFACE: u8 = 1;
const EP_BULK_IN: u8 = 0x82;
const PROBE: u16 = 1;
const COMMIT: u16 = 2;
const SET_CUR: u8 = 0x01;
const GET_CUR: u8 = 0x81;
const T: Duration = Duration::from_millis(2000);

/// One decoded video frame, tightly packed RGBA8 row-major.
pub struct Frame {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
    pub mjpeg: Option<Vec<u8>>, // raw JPEG payload when the format is MJPEG
    pub seq: u64,
}

/// What to stream: format index 1 (MJPEG) or 2 (YUY2), frame index, interval (100 ns).
#[derive(Clone, Copy, Debug)]
pub struct Mode {
    pub format: u8,
    pub frame: u8,
    pub interval: u32,
    pub bitrate: u32,
    pub max_frame: u32,
}

// From the live decoded VS frame table (lsusb): fmt1 MJPEG f1=1920x1080
// (buf 4147200, intervals 166666..666666) f2=3840x2160 f3=1280x720
// (buf 1843200); fmt2 YUY2 f1=640x360 (buf 460800) f2=640x480.
pub const MJPEG_1080: Mode = Mode { format: 1, frame: 1, interval: 333333, bitrate: 40_000_000, max_frame: 4_147_200 };
pub const MJPEG_720: Mode = Mode { format: 1, frame: 3, interval: 333333, bitrate: 20_000_000, max_frame: 1_843_200 };
pub const YUY2_360: Mode = Mode { format: 2, frame: 1, interval: 333333, bitrate: 110_592_000, max_frame: 460_800 };

/// Handle to a running stream thread; dropping it stops the stream.
pub struct Stream {
    stop: Arc<AtomicBool>,
    rx: Receiver<Frame>,
    handle: Option<JoinHandle<()>>,
    pub w: u32,
    pub h: u32,
}

impl Stream {
    /// Standalone variant: opens its own handle and claims both interfaces
    /// (for video-only tests with no control transport in the process).
    pub fn open_fresh(mode: Mode, w: u32, h: u32) -> Result<Self, String> {
        let info = nusb::list_devices()
            .wait()
            .map_err(|e| format!("list: {e:?}"))?
            .into_iter()
            .find(|d| d.vendor_id() == crate::VID && d.product_id() == crate::PID)
            .ok_or("OBSBOT camera not found")?;
        let dev = info.open().wait().map_err(|e| format!("open: {e:?}"))?;
        let _vc = dev
            .detach_and_claim_interface(0)
            .wait()
            .map_err(|e| format!("claim VC: {e:?}"))?;
        // keep VC claim alive for the stream lifetime via thread-local trick:
        // simplest is to leak it into the returned struct through stop closure.
        let r = Self::start_on(dev.clone(), mode, w, h);
        std::mem::forget(_vc); // claim lives as long as the process/handle
        r
    }

    /// Open the camera's VS interface and start streaming `mode`.
    /// `dev` must be the SAME open handle whose fd claimed the VC interface
    /// (usbfs transfer rights are per-fd) — take it from
    /// `transport::NusbXu::device()`. For NOCTRL standalone tests use
    /// `Stream::open_fresh`.
    pub fn start_on(dev: Device, mode: Mode, w: u32, h: u32) -> Result<Self, String> {
        // Claim VS (same fd: transfer rights already held).
        let _iface = dev
            .detach_and_claim_interface(VS_IFACE)
            .wait()
            .map_err(|e| format!("claim VS: {e:?}"))?;


        // GET_CUR probe first — but many firmwares (this one included) answer
        // with 34 ZERO bytes (STALL). Build the struct explicitly from the
        // frame-table values instead (libdev does the same).
        let mut probe = dev
            .control_in(
                ControlIn {
                    control_type: ControlType::Class,
                    recipient: Recipient::Interface,
                    request: GET_CUR,
                    value: PROBE << 8,
                    index: (VS_IFACE as u16) << 8,
                    length: 34,
                },
                T,
            )
            .wait()
            .map_err(|e| format!("probe get: {e:?}"))?;
        // UVC 1.1 VS_PROBE_AND_COMMIT (input header bLength=15 ⇒ 1.1: bmHint
        // is 3 BYTES, shifting every field +1 vs the 1.0 layout):
        //   [0]bLength(34) [1..4]bmHint [4]bFormatIndex [5]bFrameIndex
        //   [6..10]dwFrameInterval [10..14]dwBitRate [14..18]dwMaxFrameBufSize
        //   [18..22]dwMaxPayloadTransferSize [22..26]dwClockFrequency
        //   [26]bmFramingInfo [27]bPrefered [28]bMin [29]bMax [30]bEndpointSupport
        if probe.iter().all(|b| *b == 0) {
            println!("GET_CUR probe returned zeros (stalled) — building explicit struct");
            probe = vec![0u8; 34];
        }
        probe = vec![0u8; 34];
        probe[0] = 34;
        probe[1] = 0x07; // bmHint 24-bit: bFormatIndex+bFrameIndex+dwFrameInterval valid
        probe[4] = mode.format;
        probe[5] = mode.frame;
        probe[6..10].copy_from_slice(&mode.interval.to_le_bytes());
        probe[10..14].copy_from_slice(&mode.bitrate.to_le_bytes());
        probe[14..18].copy_from_slice(&mode.max_frame.to_le_bytes());
        probe[18..22].copy_from_slice(&512u32.to_le_bytes()); // payload xfer = mps
        probe[26] = 0x01; // bmFramingInfo: StreamInterval supported
        probe[27] = 1; // bPreferedVersion
        probe[28] = 1; // bMinVersion
        probe[29] = 1; // bMaxVersion
        probe[30] = 0x04; // bEndpointSupport: bulk
        set_vs(&dev, PROBE as u8, 0, &probe)?;

        let commit = probe.clone();
        // Stream-on switch is wValue low byte (bCommand = SET_CUR), NOT payload.
        set_vs(&dev, COMMIT as u8, 1, &commit)?;

        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel::<Frame>();
        let stop2 = stop.clone();
        let is_mjpeg = mode.format == 1;
        let handle = std::thread::spawn(move || {
            let mut reader = match _iface.endpoint::<Bulk, In>(EP_BULK_IN) {
                Ok(ep) => ep.reader(16 * 1024),
                Err(e) => {
                    eprintln!("stream ep: {e:?}");
                    return;
                }
            };
            let mut buf = vec![0u8; 64 * 1024];
            let mut cur_fid: i32 = -1;
            let mut acc: Vec<u8> = Vec::with_capacity(8 << 20);
            let mut seq = 0u64;
            let mut reads = 0u64;
            loop {
                let n = match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                reads += 1;
                if reads == 1 {
                    let show = n.min(24);
                    eprintln!(
                        "first bulk read {n}B: {}",
                        buf[..show].iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
                    );
                }
                if n < 2 {
                    continue;
                }
                let b0 = buf[0];
                // bmHeaderInfo: bit0 STI-valid, bit1 SCR-valid (0..1 legal),
                // bit2 EOH, bit3 ERR, bit4 FID. bits0-1 == 0b11 is reserved.
                if b0 & 0x03 == 0x03 {
                    continue; // reserved header form
                }
                let hdr_len = buf[1] as usize; // bHeaderLength IS in bytes
                if hdr_len > n {
                    continue;
                }
                let fid = (b0 >> 4) & 1;
                let eof = (b0 >> 2) & 1;
                if fid as i32 != cur_fid {
                    acc.clear();
                    cur_fid = fid as i32;
                }
                acc.extend_from_slice(&buf[hdr_len..n]);
                if eof == 1 {
                    let payload = std::mem::take(&mut acc);
                    acc = Vec::with_capacity(8 << 20);
                    if let Some(f) = decode_frame(is_mjpeg, w, h, &payload, seq) {
                        seq += 1;
                        if tx.send(f).is_err() {
                            break;
                        }
                    }
                }
                if stop2.load(Ordering::Relaxed) {
                    break;
                }
            }
        });

        Ok(Self { stop, rx, handle: Some(handle), w, h })
    }

    /// Blocking receive of the next decoded frame.
    pub fn recv(&self) -> Option<Frame> {
        self.rx.recv().ok()
    }

    /// Non-blocking-ish receive for diagnostics: `None` on timeout (channel alive).
    pub fn recv_timeout(&self, d: std::time::Duration) -> Result<Option<Frame>, String> {
        match self.rx.recv_timeout(d) {
            Ok(f) => Ok(Some(f)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                Err("stream thread ended (channel closed)".into())
            }
        }
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h; // reader unblocks when the interface drops
        }
    }
}

fn b1_len(b1: &u8) -> u8 {
    b1 & 0x3f
}

fn set_vs(dev: &Device, selector: u8, value_low: u8, data: &[u8]) -> Result<(), String> {
    dev.control_out(
        ControlOut {
            control_type: ControlType::Class,
            recipient: Recipient::Interface,
            request: SET_CUR,
            value: ((selector as u16) << 8) | value_low as u16,
            index: (VS_IFACE as u16) << 8,
            data,
        },
        T,
    )
    .wait()
    .map_err(|e| format!("probe/commit set sel{selector}: {e:?}"))?;
    Ok(())
}

fn decode_frame(is_mjpeg: bool, w: u32, h: u32, payload: &[u8], seq: u64) -> Option<Frame> {
    if is_mjpeg {
        let mut dec = jpeg_decoder::Decoder::new(payload);
        match dec.decode() {
            Ok(samples) => {
                let info = dec.info()?;
                let iw = info.width as u32;
                let ih = info.height as u32;
                let px = info.pixel_format.pixel_bytes();
                let mut rgba = Vec::with_capacity((samples.len() / px).max(1) * 4);
                for c in samples.chunks_exact(px) {
                    match px {
                        3 => rgba.extend_from_slice(&[c[0], c[1], c[2], 255]),
                        4 => rgba.extend_from_slice(&[c[0], c[1], c[2], 255]), // CMYK: drop K
                        1 => rgba.extend_from_slice(&[c[0], c[0], c[0], 255]), // L8
                        2 => rgba.extend_from_slice(&[c[0], c[0], c[0], 255]), // L16 hi
                        _ => rgba.extend_from_slice(&[0, 0, 0, 255]),
                    }
                }
                Some(Frame { w: iw, h: ih, rgba, mjpeg: Some(payload.to_vec()), seq })
            }
            Err(e) => {
                eprintln!("jpeg: {e:?} ({} B)", payload.len());
                None
            }
        }
    } else {
        // YUY2: 2 bytes/px, 4 bytes = 2 px (Y0 U Y1 V)
        if payload.len() < (w * h * 2) as usize {
            return None;
        }
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        let mut o = 0usize;
        for px in payload[..(w * h * 2) as usize].chunks_exact(4) {
            let (y0, u, y1, v) = (px[0] as i32, px[1] as i32 - 128, px[2] as i32, px[3] as i32 - 128);
            for y in [y0, y1] {
                let c = 298 * (y - 16) + 409 * (v - 128) + 128;
                let r = (c >> 8).clamp(0, 255) as u8;
                let c2 = 298 * (y - 16) - 100 * (u - 128) - 208 * (v - 128) + 128;
                let g = (c2 >> 8).clamp(0, 255) as u8;
                let c3 = 298 * (y - 16) + 516 * (u - 128) + 128;
                let b = (c3 >> 8).clamp(0, 255) as u8;
                rgba[o] = r;
                rgba[o + 1] = g;
                rgba[o + 2] = b;
                rgba[o + 3] = 255;
                o += 4;
            }
        }
        Some(Frame { w, h, rgba, mjpeg: None, seq })
    }
}
