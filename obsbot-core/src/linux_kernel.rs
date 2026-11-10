// Kernel-path transport + capture (Linux/WSL).
//
// The kernel uvcvideo driver negotiates UVC streaming correctly on this
// camera (our userspace probe/commit never turned the bulk pipe on), and it
// exposes raw XU passthrough via UVCIOC_CTRL_QUERY — no v4l-subdev nodes
// needed. So on WSL: uvcvideo owns the USB device, and we do
//   * XU AA frames      -> UVCIOC_CTRL_QUERY (GET_LEN/SET_CUR/GET_CUR)
//   * MJPEG/YUY2 frames -> V4L2 mmap on the same capture node.
//
// Structs are hand-rolled against <linux/videodev2.h> / <linux/uvcvideo.h>
// 6.x (stable ABI); no extra crates needed beyond libc.
#![cfg(target_os = "linux")]

use crate::device::{Result, XuTransport};
use std::os::unix::io::RawFd;

macro_rules! ioctl {
    ($dir:expr, $nr:expr, $size:expr) => {
        (($dir as u64) << 30 | ($size as u64) << 16 | ((b'V' as u64) << 8) | $nr as u64)
            as libc::c_ulong
    };
}
const IOC_NONE: u64 = 0;
const IOC_WRITE: u64 = 1;
const IOC_READ: u64 = 2;
macro_rules! iow { ($nr:expr, $s:expr) => { ioctl!(IOC_WRITE, $nr, $s) }; }
macro_rules! ior { ($nr:expr, $s:expr) => { ioctl!(IOC_READ, $nr, $s) }; }
macro_rules! iowr { ($nr:expr, $s:expr) => { ioctl!(IOC_READ | IOC_WRITE, $nr, $s) }; }

// --- XU passthrough (linux/uvcvideo.h) -------------------------------------

// struct uvc_xu_control_query { u8 unit; u8 selector; u8 query; u16 size; u8 *data; }
// -> 16 bytes on x86_64 (pad after `query`, pointer aligned to 8).
#[repr(C)]
struct UvcXuQuery {
    unit: u8,
    selector: u8,
    query: u8,
    size: u16,
    data: *mut u8,
}

const UVCIOC_CTRL_QUERY: libc::c_ulong = ((IOC_READ | IOC_WRITE) << 30
    | (std::mem::size_of::<UvcXuQuery>() as u64) << 16
    | ((b'u' as u64) << 8)
    | 0x21) as libc::c_ulong;

const UVC_SET_CUR: u8 = 0x01;
const UVC_GET_CUR: u8 = 0x81;
const UVC_GET_LEN: u8 = 0x85;

#[repr(C)]
struct V4l2Capability {
    driver: [u8; 16],
    card: [u8; 32],
    bus_info: [u8; 32],
    version: u32,
    capabilities: u32,
    device_caps: u32,
    reserved: [u32; 3],
}
const VIDIOC_QUERYCAP: libc::c_ulong = ior!(0, std::mem::size_of::<V4l2Capability>());
const V4L2_CAP_VIDEO_CAPTURE: u32 = 0x0000_0001;

/// Open the first /dev/videoN whose card name contains `needle` and which is
/// a real capture node (skips the uvc metadata node).
fn open_uvc_node(needle: &[u8]) -> Result<RawFd> {
    for n in 0..16 {
        let path = format!("/dev/video{n}\0");
        let fd = unsafe {
            libc::open(path.as_ptr() as *const libc::c_char, libc::O_RDWR | libc::O_CLOEXEC)
        };
        if fd < 0 {
            continue;
        }
        let mut cap = V4l2Capability {
            driver: [0; 16],
            card: [0; 32],
            bus_info: [0; 32],
            version: 0,
            capabilities: 0,
            device_caps: 0,
            reserved: [0; 3],
        };
        let r = unsafe { libc::ioctl(fd, VIDIOC_QUERYCAP, &mut cap as *mut V4l2Capability) };
        if r == 0 && (cap.device_caps & V4L2_CAP_VIDEO_CAPTURE != 0) {
            let end = cap.card.iter().position(|b| *b == 0).unwrap_or(32);
            if cap.card[..end]
                .to_ascii_uppercase()
                .windows(needle.len().max(1))
                .any(|w| w.eq_ignore_ascii_case(needle))
            {
                return Ok(fd);
            }
        }
        unsafe { libc::close(fd) };
    }
    Err("no uvcvideo capture node for OBSBOT camera (bound? permissions?)".into())
}

/// XU control over a uvcvideo capture node — identical AA frames/protocol as
/// the nusb transport. Needs access to /dev/videoN (root or video group).
pub struct V4lXu {
    fd: RawFd,
}

impl V4lXu {
    pub fn open_camera() -> Result<Self> {
        Ok(Self { fd: open_uvc_node(b"OBSBOT")? })
    }

    fn xu(&self, request: u8, selector: u8, buf: &mut [u8]) -> Result<usize> {
        let mut q = UvcXuQuery {
            unit: 2, // XU bUnitID (VideoControl iface 0, unit 2)
            selector,
            query: request,
            size: buf.len() as u16,
            data: buf.as_mut_ptr(),
        };
        let r = unsafe { libc::ioctl(self.fd, UVCIOC_CTRL_QUERY, &mut q as *mut UvcXuQuery) };
        if r != 0 {
            return Err(format!(
                "UVCIOC_CTRL_QUERY(req={request:#04x} sel={selector}): {}",
                std::io::Error::last_os_error()
            )
            .into());
        }
        Ok(q.size as usize)
    }
}

impl XuTransport for V4lXu {
    fn get_len(&mut self, selector: u8) -> Result<u16> {
        let mut b = [0u8; 2];
        self.xu(UVC_GET_LEN, selector, &mut b)?;
        Ok(u16::from_le_bytes(b))
    }
    fn set_cur(&mut self, selector: u8, data: &[u8]) -> Result<()> {
        let mut b = data.to_vec();
        self.xu(UVC_SET_CUR, selector, &mut b)?;
        Ok(())
    }
    fn get_cur(&mut self, selector: u8, len: usize) -> Result<Vec<u8>> {
        let mut b = vec![0u8; len];
        let n = self.xu(UVC_GET_CUR, selector, &mut b)?;
        b.truncate(n.min(len));
        Ok(b)
    }
    /// The kernel drains the async EP 0x86 interrupt pipe itself; this path
    /// surfaces no raw event frames, so control code just doesn't see them.
    fn poll_event(&mut self) -> Option<Vec<u8>> {
        None
    }
}

impl Drop for V4lXu {
    fn drop(&mut self) {
        unsafe { libc::close(self.fd) };
    }
}

// --- V4L2 capture -----------------------------------------------------------

const V4L2_BUF_TYPE_VIDEO_CAPTURE: u32 = 1;
const V4L2_MEMORY_MMAP: u32 = 1;
const V4L2_FIELD_NONE: u32 = 1;
pub const PIX_MJPEG: u32 = u32::from_le_bytes(*b"MJPG");
pub const PIX_YUYV: u32 = u32::from_le_bytes(*b"YUYV");

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct V4l2PixFormat {
    width: u32,
    height: u32,
    pixelformat: u32,
    field: u32,
    bytesperline: u32,
    sizeimage: u32,
    colorspace: u32,
    priv_: u32,
    reserved: [u32; 4],
}

// Sizes/offsets verified against <linux/videodev2.h> with scratch/abiprobe.c
// on this WSL kernel: v4l2_format 204, v4l2_requestbuffers {count,type,memory}
// 20, v4l2_buffer 88 (memory@60, m.offset@64, length@72), v4l2_streamparm 204.
#[repr(C)]
#[derive(Clone, Copy)]
struct V4l2Format {
    ftype: u32,
    fmt: V4l2PixFormat,
    reserved: [u32; 39], // pad union to 208 B (u64-aligned) as the kernel expects
}
impl Default for V4l2Format {
    fn default() -> Self {
        Self { ftype: 0, fmt: V4l2PixFormat::default(), reserved: [0; 39] }
    }
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct V4l2RequestBuffers {
    count: u32,
    type_: u32,
    memory: u32,
    reserved: [u32; 2],
}

#[repr(C)]
#[derive(Default)]
struct V4l2Buffer {
    index: u32,
    type_: u32,
    bytesused: u32,
    flags: u32,
    field: u32,
    pad0: u32,
    timestamp_s: i64,
    timestamp_us: i64,
    pad1: [u8; 20], // timestamp2 union + reserved
    memory: u32,
    m_offset: u32,
    pad2: u32,
    length: u32,
    pad3: u64,
}

#[repr(C)]
#[derive(Clone)]
struct V4l2StParm {
    ptype: u32,
    capability: u32,     // v4l2_capture_parm lives at union offset 0
    capture_capabilities: u32,
    timeperframe_num: u32,
    timeperframe_den: u32,
    extendedmode: u32,
    readbuffers: u32,
    pad: [u8; 176], // pad union to 204 B
}
impl Default for V4l2StParm {
    fn default() -> Self {
        Self {
            ptype: 0,
            capability: 0,
            capture_capabilities: 0,
            timeperframe_num: 0,
            timeperframe_den: 0,
            extendedmode: 0,
            readbuffers: 0,
            pad: [0; 176],
        }
    }
}

const VIDIOC_S_FMT: libc::c_ulong = iowr!(5, std::mem::size_of::<V4l2Format>());
const VIDIOC_REQBUFS: libc::c_ulong = iowr!(8, std::mem::size_of::<V4l2RequestBuffers>());
const VIDIOC_QUERYBUF: libc::c_ulong = iowr!(9, std::mem::size_of::<V4l2Buffer>());
const VIDIOC_QBUF: libc::c_ulong = iowr!(15, std::mem::size_of::<V4l2Buffer>());
const VIDIOC_DQBUF: libc::c_ulong = iowr!(17, std::mem::size_of::<V4l2Buffer>());
// This header numbers them 18/19 (older headers used 20/21 — verify before "fixing").
const VIDIOC_STREAMON: libc::c_ulong = iow!(18, 4usize);
const VIDIOC_STREAMOFF: libc::c_ulong = iow!(19, 4usize);
const VIDIOC_S_PARM: libc::c_ulong = iowr!(22, std::mem::size_of::<V4l2StParm>());
// v4l2_control is 8 bytes {u32 id; s32 value}; this header defines both as _IOWR
const VIDIOC_G_CTRL: libc::c_ulong = iowr!(27, 8usize);
const VIDIOC_S_CTRL: libc::c_ulong = iowr!(28, 8usize);
const VIDIOC_QUERYCTRL: libc::c_ulong = iowr!(36, std::mem::size_of::<V4l2QueryCtrl>());
const V4L2_CTRL_FLAG_DISABLED: u32 = 0x0001;

// v4l2_queryctrl is 68 bytes on this kernel (name[32] at offset 8,
// minimum at 40). Checked against <linux/videodev2.h>.
#[repr(C)]
struct V4l2QueryCtrl {
    id: u32,
    type_: u32,
    name: [u8; 32],
    minimum: i32,
    maximum: i32,
    step: i32,
    default_value: i32,
    flags: u32,
    reserved: [u32; 2],
}
const _: () = assert!(std::mem::size_of::<V4l2QueryCtrl>() == 68);
const _: () = assert!(std::mem::size_of::<V4l2StParm>() == 204);

impl V4l2QueryCtrl {
    fn zero() -> Self {
        Self {
            id: 0,
            type_: 0,
            name: [0; 32],
            minimum: 0,
            maximum: 0,
            step: 0,
            default_value: 0,
            flags: 0,
            reserved: [0; 2],
        }
    }
}

struct CaptureRate {
    fps: f32,
    numerator: u32,
    denominator: u32,
}

/// `denominator/numerator` frames per second. `None` when either side is 0.
fn frames_per_second(numerator: u32, denominator: u32) -> Option<f32> {
    if numerator == 0 || denominator == 0 {
        None
    } else {
        Some(denominator as f32 / numerator as f32)
    }
}

/// `VIDIOC_S_PARM` for a capture stream. Fails when the driver will not
/// commit `fps` (the Tiny 2 Lite MJPEG default is 30 even at 720p/1080p).
fn set_capture_rate(fd: RawFd, fps: u32) -> Result<CaptureRate> {
    let want = fps.max(1);
    let mut sp = V4l2StParm {
        ptype: V4L2_BUF_TYPE_VIDEO_CAPTURE,
        timeperframe_num: 1,
        timeperframe_den: want,
        ..Default::default()
    };
    unsafe { libc::ioctl(fd, VIDIOC_S_PARM, &mut sp as *mut V4l2StParm) }.check("S_PARM")?;
    let got = frames_per_second(sp.timeperframe_num, sp.timeperframe_den).ok_or_else(|| {
        format!(
            "S_PARM returned interval {}/{}",
            sp.timeperframe_num, sp.timeperframe_den
        )
    })?;
    if (got - want as f32).abs() > 1.5 {
        return Err(format!(
            "camera negotiated {got:.2} fps (interval {}/{}), wanted {want}",
            sp.timeperframe_num, sp.timeperframe_den
        )
        .into());
    }
    Ok(CaptureRate {
        fps: got,
        numerator: sp.timeperframe_num,
        denominator: sp.timeperframe_den,
    })
}
// V4L2 user-class CIDs the kernel uvcvideo maps the camera's UVC controls onto.
pub const CID_PAN_ABSOLUTE: u32 = 0x009a_0908;
pub const CID_TILT_ABSOLUTE: u32 = 0x009a_0909;
pub const CID_ZOOM_ABSOLUTE: u32 = 0x009a_090d;
pub const CID_ZOOM_CONTINUOUS: u32 = 0x009a_090f;
pub const CID_PAN_SPEED: u32 = 0x009a_0920;
pub const CID_TILT_SPEED: u32 = 0x009a_0921;
/// Processing-unit controls (`V4L2_CID_BASE`).
pub const CID_CONTRAST: u32 = 0x0098_0901;
pub const CID_SATURATION: u32 = 0x0098_0902;
pub const CID_HUE: u32 = 0x0098_0903;
pub const CID_AUTO_WHITE_BALANCE: u32 = 0x0098_090c;
pub const CID_POWER_LINE_FREQUENCY: u32 = 0x0098_0918;
pub const CID_WHITE_BALANCE_TEMPERATURE: u32 = 0x0098_091a;
pub const CID_SHARPNESS: u32 = 0x0098_091b;
pub const CID_BACKLIGHT_COMPENSATION: u32 = 0x0098_091c;
/// Camera-class controls. Exposure auto uses 0 = auto, 1 = manual.
pub const CID_EXPOSURE_AUTO: u32 = 0x009a_0901;
pub const CID_FOCUS_AUTO: u32 = 0x009a_090c;

/// `VIDIOC_QUERYCTRL` result for one V4L2 control (zoom, and so on).
#[derive(Clone, Copy, Debug)]
pub struct CtrlRange {
    pub minimum: i32,
    pub maximum: i32,
    pub step: i32,
    pub default_value: i32,
}

/// Running V4L2 capture on the OBSBOT node. Drop stops streaming and frees
/// every mapping.
pub struct Capture {
    fd: RawFd,
    bufs: Vec<(*mut libc::c_void, u32)>, // (mmap ptr, length)
    acc: Vec<u8>,                        // MJPEG fragment reassembly buffer
    pub w: u32,
    pub h: u32,
    pub mjpeg: bool,
    /// Rate the driver committed, in frames per second.
    /// uvcvideo reports this as `dwFrameInterval/10_000_000` (60 fps is
    /// `166666/10000000`), not the `1/fps` fraction we asked for.
    pub fps: f32,
    pub interval_num: u32,
    pub interval_den: u32,
}

impl Capture {
    pub fn start_mjpeg(w: u32, h: u32) -> Result<Self> {
        Self::start(w, h, PIX_MJPEG, 30)
    }
    /// MJPEG at an explicit frame rate. The camera's MJPEG descriptors
    /// advertise 60 fps (interval 166666 us) at 1280x720 and 1920x1080.
    pub fn start_mjpeg_fps(w: u32, h: u32, fps: u32) -> Result<Self> {
        Self::start(w, h, PIX_MJPEG, fps)
    }
    pub fn start_yuyv(w: u32, h: u32) -> Result<Self> {
        Self::start(w, h, PIX_YUYV, 30)
    }

    fn start(w: u32, h: u32, pix: u32, fps: u32) -> Result<Self> {
        let fd = open_uvc_node(b"OBSBOT")?;

        let mut f = V4l2Format::default();
        f.ftype = V4L2_BUF_TYPE_VIDEO_CAPTURE;
        f.fmt.width = w;
        f.fmt.height = h;
        f.fmt.pixelformat = pix;
        f.fmt.field = V4L2_FIELD_NONE;
        unsafe { libc::ioctl(fd, VIDIOC_S_FMT, &mut f as *mut V4l2Format) }.check("S_FMT")?;

        // Commit the frame interval before STREAMON. The descriptor default
        // is 333333 (30 fps) even on frames that also list 166666 (60 fps).
        // Do not set the old 0x8000_0000 "write" flag: v4l2_check_streamparm
        // rejects it for capture (EINVAL) and the interval stays at 30.
        // uvcvideo rewrites the fraction in place; a discarded ioctl is how
        // the preview used to stay at 30 fps with no error.
        let rate = match set_capture_rate(fd, fps) {
            Ok(rate) => rate,
            Err(e) => {
                unsafe { libc::close(fd) };
                return Err(e);
            }
        };

        let mut rb = V4l2RequestBuffers {
            count: 4,
            type_: V4L2_BUF_TYPE_VIDEO_CAPTURE,
            memory: V4L2_MEMORY_MMAP,
            reserved: [0; 2],
        };
        unsafe { libc::ioctl(fd, VIDIOC_REQBUFS, &mut rb as *mut V4l2RequestBuffers) }
            .check("REQBUFS")?;
        let nbufs = rb.count.clamp(2, 4) as usize;

        let mut bufs = Vec::new();
        for i in 0..nbufs {
            let mut b = V4l2Buffer {
                index: i as u32,
                type_: V4L2_BUF_TYPE_VIDEO_CAPTURE,
                memory: V4L2_MEMORY_MMAP,
                ..Default::default()
            };
            unsafe { libc::ioctl(fd, VIDIOC_QUERYBUF, &mut b as *mut V4l2Buffer) }
                .check("QUERYBUF")?;
            let ptr = unsafe {
                libc::mmap(
                    std::ptr::null_mut(),
                    b.length as usize,
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_SHARED,
                    fd,
                    b.m_offset as libc::off_t,
                )
            };
            if ptr == libc::MAP_FAILED {
                return Err("mmap capture buffer failed".into());
            }
            bufs.push((ptr, b.length));
            unsafe { libc::ioctl(fd, VIDIOC_QBUF, &mut b as *mut V4l2Buffer) }.check("QBUF")?;
        }

        let t = V4L2_BUF_TYPE_VIDEO_CAPTURE;
        unsafe { libc::ioctl(fd, VIDIOC_STREAMON, &t as *const u32) }.check("STREAMON")?;
        Ok(Self {
            fd,
            bufs,
            acc: Vec::new(),
            w: f.fmt.width,
            h: f.fmt.height,
            mjpeg: pix == PIX_MJPEG,
            fps: rate.fps,
            interval_num: rate.numerator,
            interval_den: rate.denominator,
        })
    }

    /// Blocking dequeue of one frame: raw MJPEG bytes (or YUY2 rows) plus a
    /// nanosecond timestamp. Buffers are re-queued automatically.
    ///
    /// uvcvideo strips the UVC payload header itself — `bytesused` is pure
    /// image payload (the header goes out the separate metadata node).
    /// For MJPEG, OBSBOT splits full frames across 32 KB buffers, so this
    /// reassembles from a JPEG SOI up to and including the trailing EOI,
    /// skipping mid-frame fragments that arrive without a leading SOI.
    pub fn next_frame(&mut self) -> Result<(Vec<u8>, i64)> {
        loop {
            let mut b = V4l2Buffer {
                type_: V4L2_BUF_TYPE_VIDEO_CAPTURE,
                memory: V4L2_MEMORY_MMAP,
                ..Default::default()
            };
            unsafe { libc::ioctl(self.fd, VIDIOC_DQBUF, &mut b as *mut V4l2Buffer) }
                .check("DQBUF")?;
            let (ptr, _) = self.bufs[b.index as usize];
            let data = unsafe {
                std::slice::from_raw_parts(ptr.cast::<u8>(), b.bytesused as usize)
            }
            .to_vec();
            unsafe { libc::ioctl(self.fd, VIDIOC_QBUF, &mut b as *mut V4l2Buffer) };
            let ts = b.timestamp_s * 1_000_000_000 + b.timestamp_us * 1_000;

            if !self.mjpeg {
                return Ok((data, ts));
            }
            let complete = |d: &[u8]| -> bool {
                d.len() > 3 && d[0] == 0xFF && d[1] == 0xD8 && d.ends_with(&[0xFF, 0xD9])
            };
            if complete(&data) {
                return Ok((data, ts));
            }
            // partial: keep accumulating fragments until an EOI shows up
            if data.starts_with(&[0xFF, 0xD8]) {
                self.acc = data;
            } else if self.acc.is_empty() {
                continue; // mid-frame fragment, nothing to append to
            } else {
                self.acc.extend_from_slice(&data);
            }
            if complete(&self.acc) {
                return Ok((std::mem::take(&mut self.acc), ts));
            }
            if self.acc.len() > 16 * 1024 * 1024 {
                self.acc.clear(); // resync on corruption
            }
        }
    }

    /// VIDIOC_G_CTRL — read one camera/class control (e.g. zoom_absolute).
    pub fn get_ctrl(&self, id: u32) -> Result<i32> {
        get_ctrl_fd(self.fd, id)
    }

    /// VIDIOC_S_CTRL — write one camera/class control.
    pub fn set_ctrl(&self, id: u32, value: i32) -> Result<()> {
        set_ctrl_fd(self.fd, id, value)
    }

    /// `VIDIOC_QUERYCTRL` — min/max/step/default for one control id.
    pub fn query_ctrl(&self, id: u32) -> Result<CtrlRange> {
        query_ctrl_fd(self.fd, id)
    }

    /// `VIDIOC_STREAMOFF`. Mappings stay valid. The next `resume` queues
    /// the buffers again — STREAMOFF drops them from the driver's queue.
    ///
    /// Selector-6 XU writes while this stream is running at 60 fps stall
    /// the usbip link (isochronous video times out, then the vhci socket
    /// closes). Callers stop the stream, write the control, then resume.
    pub fn suspend(&mut self) -> Result<()> {
        let t = V4L2_BUF_TYPE_VIDEO_CAPTURE;
        unsafe { libc::ioctl(self.fd, VIDIOC_STREAMOFF, &t as *const u32) }.check("STREAMOFF")?;
        Ok(())
    }

    /// Re-queue every mapped buffer and `VIDIOC_STREAMON` at 30 fps.
    ///
    /// Coming back at 60 fps is what closed the usbip socket: at 14:42:59
    /// the commit control timed out (-62) about a second after STREAMOFF,
    /// after the unlink drain had already finished. 30 fps is the interval
    /// the camera advertises as its default.
    pub fn resume(&mut self) -> Result<()> {
        let rate = set_capture_rate(self.fd, 30)?;
        self.fps = rate.fps;
        self.interval_num = rate.numerator;
        self.interval_den = rate.denominator;
        for index in 0..self.bufs.len() {
            let mut b = V4l2Buffer {
                index: index as u32,
                type_: V4L2_BUF_TYPE_VIDEO_CAPTURE,
                memory: V4L2_MEMORY_MMAP,
                ..Default::default()
            };
            unsafe { libc::ioctl(self.fd, VIDIOC_QBUF, &mut b as *mut V4l2Buffer) }
                .check("QBUF")?;
        }
        let t = V4L2_BUF_TYPE_VIDEO_CAPTURE;
        unsafe { libc::ioctl(self.fd, VIDIOC_STREAMON, &t as *const u32) }.check("STREAMON")?;
        Ok(())
    }
}

/// A second open of the capture node used only for pan, tilt, and zoom.
///
/// Control writes stay off the streaming fd so they are not waiting on the
/// next captured frame. This is the same kind of open `v4l2-ctl` uses.
pub struct CtrlNode {
    fd: RawFd,
}

impl CtrlNode {
    pub fn open() -> Result<Self> {
        Ok(Self {
            fd: open_uvc_node(b"OBSBOT")?,
        })
    }

    pub fn get_ctrl(&self, id: u32) -> Result<i32> {
        get_ctrl_fd(self.fd, id)
    }

    pub fn set_ctrl(&self, id: u32, value: i32) -> Result<()> {
        set_ctrl_fd(self.fd, id, value)
    }

    pub fn query_ctrl(&self, id: u32) -> Result<CtrlRange> {
        query_ctrl_fd(self.fd, id)
    }
}

impl Drop for CtrlNode {
    fn drop(&mut self) {
        unsafe { libc::close(self.fd) };
    }
}

#[repr(C)]
struct V4l2Ctrl {
    id: u32,
    value: i32,
}

fn get_ctrl_fd(fd: RawFd, id: u32) -> Result<i32> {
    let mut c = V4l2Ctrl { id, value: 0 };
    unsafe { libc::ioctl(fd, VIDIOC_G_CTRL, &mut c as *mut V4l2Ctrl) }.check("G_CTRL")?;
    Ok(c.value)
}

fn set_ctrl_fd(fd: RawFd, id: u32, value: i32) -> Result<()> {
    let c = V4l2Ctrl { id, value };
    unsafe { libc::ioctl(fd, VIDIOC_S_CTRL, &c as *const V4l2Ctrl) }.check("S_CTRL")?;
    Ok(())
}

fn query_ctrl_fd(fd: RawFd, id: u32) -> Result<CtrlRange> {
    let mut q = V4l2QueryCtrl {
        id,
        ..V4l2QueryCtrl::zero()
    };
    unsafe { libc::ioctl(fd, VIDIOC_QUERYCTRL, &mut q as *mut V4l2QueryCtrl) }.check("QUERYCTRL")?;
    if (q.flags & V4L2_CTRL_FLAG_DISABLED) != 0 {
        return Err(format!("control {id:#x} is disabled").into());
    }
    if q.maximum < q.minimum {
        return Err(format!(
            "control {id:#x} range {}..{} is inverted",
            q.minimum, q.maximum
        )
        .into());
    }
    Ok(CtrlRange {
        minimum: q.minimum,
        maximum: q.maximum,
        step: q.step.max(1),
        default_value: q.default_value,
    })
}

impl Drop for Capture {
    fn drop(&mut self) {
        let t = V4L2_BUF_TYPE_VIDEO_CAPTURE;
        unsafe {
            libc::ioctl(self.fd, VIDIOC_STREAMOFF, &t as *const u32);
            for (ptr, len) in &self.bufs {
                libc::munmap(*ptr, *len as usize);
            }
            libc::close(self.fd);
        }
    }
}

trait Check {
    fn check(self, what: &str) -> Result<()>;
}
impl Check for libc::c_int {
    fn check(self, what: &str) -> Result<()> {
        if self == 0 {
            Ok(())
        } else {
            Err(format!("{what}: {}", std::io::Error::last_os_error()).into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::frames_per_second;

    #[test]
    fn interval_fraction_is_frames_per_second() {
        assert_eq!(frames_per_second(1, 60), Some(60.0));
        assert_eq!(frames_per_second(0, 60), None);
        assert_eq!(frames_per_second(60, 0), None);
        let sixty = frames_per_second(166_666, 10_000_000).unwrap();
        assert!((sixty - 60.0).abs() < 0.01, "{sixty}");
        let thirty = frames_per_second(333_333, 10_000_000).unwrap();
        assert!((thirty - 30.0).abs() < 0.01, "{thirty}");
    }
}
