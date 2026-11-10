//! Live transport for the Tiny 2 Lite: UVC Extension Unit controls via
//! `nusb` (Linux; same code paths xuprobe/replay2 proved against the real
//! camera, wrapped behind `device::XuTransport`).
//!
//! Control transfers go to the VideoControl interface (0), Extension Unit
//! bUnitID 2, XU selector in `wValue` high byte, `wIndex = 2<<8|0` —
//! byte-exact from the Windows capture (`bmRequestType/index/value`):
//!   GET_LEN 0x85 (a1)  value=sel<<8|2, len 2   → `3c 00` (60)
//!   SET_CUR 0x01 (21)  value=0x0200, 60-byte AA frame
//!   GET_CUR 0x81 (a1)  value=0x0200, len 60    → reply pull
//! EP 0x86 interrupt-IN reports (`0x89` AA event frames) are read by a
//! background thread and surfaced through `poll_event`.
//!
//! NOTE: claiming the VC interface detaches `uvcvideo` temporarily; the
//! kernel driver re-attaches when the claimed interface (and this
//! transport) is dropped.

use crate::device::{Result, TransportError, XuTransport};
use crate::{PID, VID};
use nusb::{
    transfer::{ControlIn, ControlOut, ControlType, In, Interrupt, Recipient},
    Device, MaybeFuture,
};
use std::io::Read as _;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::Duration;

const XU_UNIT: u8 = 2; // Extension Unit bUnitID
const CT_IFACE: u8 = 0; // VideoControl interface number
const EP_IN: u8 = 0x86; // VC interrupt endpoint
const CS_XU: u16 = 0x02; // XU connector (bmRequestType CS) in wValue low byte
const T: Duration = Duration::from_millis(1500);

fn stall(e: impl std::fmt::Debug) -> TransportError {
    TransportError::Stall(format!("{e:?}"))
}

pub struct NusbXu {
    dev: Device,
    rx: Receiver<Vec<u8>>,
    stop: Arc<AtomicBool>,
}

impl NusbXu {
    /// Open the first OBSBOT Tiny 2 Lite and claim the VC interface
    /// (uvcvideo is detached for the lifetime of the claim).
    pub fn open_camera() -> std::result::Result<Self, String> {
        let info = nusb::list_devices()
            .wait()
            .map_err(|e| format!("list: {e:?}"))?
            .into_iter()
            .find(|d| d.vendor_id() == VID && d.product_id() == PID)
            .ok_or_else(|| format!("OBSBOT {VID:04x}:{PID:04x} not found (usbipd attach?)"))?;
        let dev = info.open().wait().map_err(|e| format!("open: {e:?}"))?;
        let iface = dev
            .detach_and_claim_interface(CT_IFACE)
            .wait()
            .map_err(|e| format!("claim VC iface: {e:?}"))?;

        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        let stop = Arc::new(AtomicBool::new(false));
        {
            let stop = stop.clone();
            std::thread::spawn(move || {
                let _keep = iface; // holds the claim (keeps uvcvideo away)
                let Ok(ep) = _keep.endpoint::<Interrupt, In>(EP_IN) else {
                    return;
                };
                let mut rd = ep.reader(64);
                let mut buf = [0u8; 64];
                loop {
                    match rd.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if tx.send(buf[..n].to_vec()).is_err() {
                                break;
                            }
                        }
                    }
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                }
            });
        }
        Ok(Self { dev, rx, stop })
    }

    fn xu_in(&self, request: u8, selector: u8, length: u16) -> Result<Vec<u8>> {
        self.dev
            .control_in(
                ControlIn {
                    control_type: ControlType::Class,
                    recipient: Recipient::Interface,
                    request,
                    value: ((selector as u16) << 8) | CS_XU,
                    index: ((XU_UNIT as u16) << 8) | CT_IFACE as u16,
                    length,
                },
                T,
            )
            .wait()
            .map_err(stall)
    }

    /// Shared clone of the underlying device handle. usbfs grants transfer
    /// rights per-file-descriptor once any interface is claimed, so the video
    /// stream MUST ride this handle (not its own `open_camera()`).
    pub fn device(&self) -> Device {
        self.dev.clone()
    }
}

impl Drop for NusbXu {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl XuTransport for NusbXu {
    fn get_len(&mut self, selector: u8) -> Result<u16> {
        // GET_LEN announces the transfer size for a selector (2 → 60).
        let d = self.xu_in(0x85, selector, 2)?;
        if d.len() < 2 {
            return Err(TransportError::Protocol(format!(
                "GET_LEN sel{selector}: short reply {} bytes",
                d.len()
            )));
        }
        Ok(u16::from_le_bytes([d[0], d[1]]))
    }

    fn set_cur(&mut self, selector: u8, data: &[u8]) -> Result<()> {
        self.dev
            .control_out(
                ControlOut {
                    control_type: ControlType::Class,
                    recipient: Recipient::Interface,
                    request: 0x01, // SET_CUR
                    value: ((selector as u16) << 8) | CS_XU,
                    index: ((XU_UNIT as u16) << 8) | CT_IFACE as u16,
                    data,
                },
                T,
            )
            .wait()
            .map_err(stall)
    }

    fn get_cur(&mut self, selector: u8, len: usize) -> Result<Vec<u8>> {
        self.xu_in(0x81, selector, len.min(u16::MAX as usize) as u16) // GET_CUR
    }

    fn poll_event(&mut self) -> Option<Vec<u8>> {
        self.rx.try_recv().ok()
    }
}
