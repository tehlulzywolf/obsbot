//! Protocol engine over the UVC-XU vendor channel (capture-verified).
//!
//! Sequence per command — exactly what `libdev.dll` does on the wire and
//! what live replay from WSL proved the Tiny 2 Lite accepts:
//!   GET_LEN(sel=2) → `3c 00` (60)
//!   SET_CUR 60-byte AA frame
//!   GET_CUR 60-byte reply pull (reply echoes the request's cmd pair at
//!   frame bytes 10..11; replies arrive ~80 ms after the SET)
//! EP 0x86 interrupt-IN pushes async `0x89` event frames between commands;
//! the transport buffers them and the engine drains opportunistically.

use crate::cmd::{self, Req};
use crate::frame::{self, Frame, XFER_LEN};
use std::time::Duration;

/// Errors from the vendor-IO layer.
#[derive(Debug, Clone)]
pub enum TransportError {
    /// OS/USB failure (device gone, permission, timeout).
    Io(String),
    /// Device STALLed the control request (bad selector/encoding).
    Stall(String),
    /// Framing/protocol mismatch on our side.
    Protocol(String),
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(s) => write!(f, "io: {s}"),
            Self::Stall(s) => write!(f, "stall: {s}"),
            Self::Protocol(s) => write!(f, "protocol: {s}"),
        }
    }
}

impl std::error::Error for TransportError {}

impl From<String> for TransportError {
    fn from(s: String) -> Self {
        Self::Io(s)
    }
}

impl From<&str> for TransportError {
    fn from(s: &str) -> Self {
        Self::Io(s.to_string())
    }
}

pub type Result<T> = std::result::Result<T, TransportError>;

/// Vendor-channel IO. Implemented for the Tiny 2 Lite over UVC Extension
/// Unit controls (`transport::NusbXu` on Linux); mockable in tests.
pub trait XuTransport {
    /// XU GET_LEN for a selector (the device announces the transfer size;
    /// 2 → 60). Returns the announced length.
    fn get_len(&mut self, selector: u8) -> Result<u16>;
    /// XU SET_CUR carrying `data` (must be padded to the announced length).
    fn set_cur(&mut self, selector: u8, data: &[u8]) -> Result<()>;
    /// XU GET_CUR pulling `len` bytes.
    fn get_cur(&mut self, selector: u8, len: usize) -> Result<Vec<u8>>;
    /// Non-blocking pop of one buffered EP 0x86 interrupt-IN report.
    fn poll_event(&mut self) -> Option<Vec<u8>>;
}

pub struct ObsbotUsb<T: XuTransport> {
    t: T,
    next_seq: u8,
    /// Selector 6 transfer size. `GET_LEN` once; every later field write
    /// is a single `SET_CUR`. A second length query on each click was
    /// extra control traffic on the same pipe as the video stream.
    sel6_len: u16,
}

impl<T: XuTransport> ObsbotUsb<T> {
    pub fn new(t: T) -> Self {
        Self {
            t,
            next_seq: 0,
            sel6_len: 0,
        }
    }

    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.t
    }

    fn next_seq(&mut self) -> u8 {
        let s = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        s
    }

    /// GET_LEN → SET_CUR → GET_CUR for one encoded request.
    fn issue(&mut self, req: &Req) -> Result<Vec<u8>> {
        self.t.get_len(2)?;
        let wire = frame::encode(
            req.ftype.unwrap_or(0x01),
            req.cmd_pair(),
            self.next_seq(),
            req.flags,
            req.to_payload(),
        );
        self.t.set_cur(2, &wire)?;
        self.t.get_cur(2, XFER_LEN)
    }

    /// Send a request and pull its reply (matched on the echoed cmd pair).
    /// EP6 event frames matching the pair are accepted too. Retries with
    /// backoff (capture cadence: replies land within ~80 ms; app retries
    /// every ~250 ms — `libdev.dll` 1 s timeout + 1 resend).
    pub fn request(&mut self, req: &Req) -> Result<Frame> {
        let want = req.cmd_pair();
        let mut backoff = Duration::from_millis(80);
        for _ in 0..4 {
            // events may already carry the reply
            while let Some(b) = self.t.poll_event() {
                if let Some(f) = frame::decode(&b) {
                    if f.cmd == want && f.is_reply() {
                        return Ok(f);
                    }
                }
            }
            let bytes = self.issue(req)?;
            if let Some(f) = frame::decode(&bytes) {
                if f.cmd == want && f.is_reply() {
                    return Ok(f);
                }
            }
            std::thread::sleep(backoff);
            backoff = (backoff * 2).min(Duration::from_millis(400));
        }
        Err(TransportError::Protocol(
            "no matching reply (cmd pair echo never arrived)".into(),
        ))
    }

    /// Fire-and-forget command: SET_CUR + GET_CUR pull (reply ignored, but
    /// the pull keeps the device's reply queue clean for the next command).
    pub fn command(&mut self, req: &Req) -> Result<()> {
        self.t.get_len(2)?;
        let wire = frame::encode(
            req.ftype.unwrap_or(0x01),
            req.cmd_pair(),
            self.next_seq(),
            req.flags,
            req.to_payload(),
        );
        self.t.set_cur(2, &wire)?;
        self.t.get_cur(2, XFER_LEN)?;
        Ok(())
    }

    // ---- typed helpers (SDK parity) ---------------------------------------

    /// `gimbalGetAttitudeInfoR` — reply payload: f32 roll, pitch, yaw (+ status).
    pub fn attitude(&mut self) -> Result<[f32; 3]> {
        let f = self.request(&cmd::gimbal_attitude())?;
        Ok(parse_euler(&f.payload))
    }

    /// Serial-number read (pair `C8 18`) — live-verified: payload is the
    /// ASCII serial, e.g. `RMOWLHGB081JIL`.
    pub fn serial(&mut self) -> Result<String> {
        let f = self.request(&cmd::gimbal_serial())?;
        Ok(String::from_utf8_lossy(&f.payload)
            .trim_end_matches('\0')
            .to_string())
    }

    /// Read preset slot names: each `04 3B` type-`0x21` read returns one
    /// slot's ASCII name (live-verified: "Preset1", "P2", "P3"). Stops
    /// early if a slot read errors or comes back empty.
    pub fn preset_names(&mut self, count: usize) -> Result<Vec<String>> {
        let mut out = Vec::new();
        for i in 0..count {
            match self.request(&cmd::preset_name(i as u8)) {
                Ok(f) => {
                    let s = String::from_utf8_lossy(&f.payload)
                        .trim_end_matches('\0')
                        .to_string();
                    if s.is_empty() {
                        break;
                    }
                    out.push(s);
                }
                Err(e) if i > 0 => {
                    let _ = e;
                    break;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(out)
    }

    /// `gimbalSpeedCtrlR` — continuous signed speed per axis, -1.0..1.0
    /// (capture-verified cmd pair `42 C4`, flags 0x02).
    pub fn gimbal_speed(&mut self, pan: f32, tilt: f32, roll: f32) -> Result<()> {
        self.command(&cmd::gimbal_speed(pan, tilt, roll))
    }

    /// `aiSetGimbalStop`
    pub fn gimbal_stop(&mut self) -> Result<()> {
        self.command(&cmd::gimbal_stop())
    }

    /// `aiSetGimbalEulerAngleR` — absolute euler angles (deg).
    pub fn gimbal_euler(&mut self, yaw: f32, pitch: f32, roll: f32) -> Result<()> {
        self.command(&cmd::gimbal_euler(yaw, pitch, roll))
    }

    /// `gimbalRstPosR`
    pub fn gimbal_rst(&mut self, mode: u8) -> Result<()> {
        self.command(&cmd::gimbal_rst_pos(mode))
    }

    /// `aiSetEnabledR` — AI tracking on/off.
    pub fn tracking_enable(&mut self, on: bool) -> Result<()> {
        self.command(&cmd::tracking_enable(on))
    }

    /// `aiSetTrackingModeR`
    pub fn tracking_mode(&mut self, mode: u8) -> Result<()> {
        self.command(&cmd::tracking_mode(mode))
    }

    /// `aiSetAiTrackModeEnabledR` — target type (table in `cmd`).
    pub fn tracking_type(&mut self, mode: u16) -> Result<()> {
        self.command(&cmd::tracking_type(mode))
    }

    /// `aiTrgGimbalPresetR`
    pub fn preset_goto(&mut self, preset_id: u32) -> Result<()> {
        self.command(&cmd::preset_goto(preset_id))
    }

    /// `aiAddGimbalPresetR`
    pub fn preset_add(
        &mut self,
        preset_id: u32,
        x: f32,
        y: f32,
        z: f32,
        zoom: f32,
        name: &[u8],
    ) -> Result<()> {
        self.command(&cmd::preset_add(preset_id, x, y, z, zoom, name))
    }

    /// `cameraSetAutoSleepTimeR`
    pub fn auto_sleep_seconds(&mut self, seconds: u32) -> Result<()> {
        self.command(&cmd::auto_sleep(seconds))
    }

    /// `cameraSetDevRunStatusR`. `cmd::RUN_AWAKE` (1) wakes, `cmd::RUN_SLEEP` (3) sleeps.
    pub fn run_status(&mut self, status: u8) -> Result<()> {
        self.command(&cmd::run_status(status))
    }

    /// `cameraGetBootStatus` — DLL-verified wire pair `17 10`, live-verified
    /// 2026-11-07 on Tiny 2 Lite: the device accepts the pair and answers a
    /// type-09 status-ack echoing it, but this firmware fills NO dedicated
    /// boot payload (byte 12 of the ack carries only the length of the last
    /// async event frame, bytes past 16 are stale buffer). An ack therefore
    /// means "camera present and responsive" = ready.
    /// Returns true when the device acked the pair.
    pub fn boot_status(&mut self) -> Result<bool> {
        let req = cmd::boot_status();
        let want = req.cmd_pair();
        for _ in 0..3 {
            let bytes = self.issue(&req)?;
            if let Some(f) = frame::decode(&bytes) {
                let cat = f.frame_type & frame::TYPE_MASK;
                if f.cmd == want && matches!(cat, 0x09 | 0x29) {
                    return Ok(true);
                }
            }
            std::thread::sleep(Duration::from_millis(120));
        }
        Err(TransportError::Protocol(
            "boot_status: cmd pair 17 10 never echoed".into(),
        ))
    }

    /// Drain buffered EP 0x86 event frames.
    pub fn poll_events(&mut self) -> Vec<Frame> {
        let mut out = Vec::new();
        while let Some(b) = self.t.poll_event() {
            if let Some(f) = frame::decode(&b) {
                out.push(f);
            }
        }
        out
    }

    /// Raw pan/tilt push path — XU selector 4 blocks (no AA header; used
    /// for high-rate joystick streaming in the app).
    pub fn raw_pan_tilt(&mut self, block: &[u8]) -> Result<()> {
        self.t.get_len(4)?;
        self.t.set_cur(4, block)
    }

    /// AiMode struct path — XU selector 6 (raw struct, not an AA frame).
    pub fn ai_mode_raw(&mut self, buf: &[u8]) -> Result<()> {
        self.t.get_len(6)?;
        self.t.set_cur(6, buf)
    }

    /// Set an AiMode selector-6 struct field (libdev `FUN_1800087f0(dev, 6,
    /// buf)` shape): 60-byte buffer, u16 LE field id at bytes 0..1, raw value
    /// from byte 2, zeros elsewhere. Known fields: `0x0100` MediaMode,
    /// `0x0101` WDR/HDR, `0x0103` FaceAE, `0x1021` FieldView (4 x u32).
    pub fn ai_field(&mut self, field: u16, val: &[u8]) -> Result<()> {
        if self.sel6_len == 0 {
            self.sel6_len = self.t.get_len(6)?;
        }
        let n = (self.sel6_len as usize).max(16);
        let mut b = vec![0u8; n];
        b[0..2].copy_from_slice(&field.to_le_bytes());
        let m = val.len().min(n - 2);
        b[2..2 + m].copy_from_slice(&val[..m]);
        self.t.set_cur(6, &b)
    }

    /// HDR toggle (roadmap item 2). `aa` = force the AA pair `14 10`;
    /// default rides AiMode sel6 field `0x0101` (the dev_mode-0 path
    /// cameraSetWdrR takes for this product family).
    pub fn hdr(&mut self, on: bool, aa: bool) -> Result<()> {
        if aa {
            self.command(&crate::cmd::wdr_aa(on))
        } else {
            self.ai_field(0x0101, &[on as u8])
        }
    }

    pub fn face_focus(&mut self, on: bool) -> Result<()> {
        self.command(&crate::cmd::face_focus(on))
    }

    pub fn set_af_mode(&mut self, m: u32) -> Result<()> {
        self.command(&crate::cmd::set_af_mode(m))
    }

    pub fn af_mode(&mut self) -> Result<u32> {
        let f = self.request(&crate::cmd::get_af_mode())?;
        Ok(parse_u32(&f.payload))
    }

    pub fn set_exp_mode(&mut self, m: u32) -> Result<()> {
        self.command(&crate::cmd::set_exp_mode(m))
    }

    pub fn exp_mode(&mut self) -> Result<u32> {
        let f = self.request(&crate::cmd::get_exp_mode())?;
        Ok(parse_u32(&f.payload))
    }

    pub fn set_mirror_flip(&mut self, v: u32) -> Result<()> {
        self.command(&crate::cmd::set_mirror_flip(v))
    }

    pub fn mirror_flip(&mut self) -> Result<u32> {
        let f = self.request(&crate::cmd::get_mirror_flip())?;
        Ok(parse_u32(&f.payload))
    }
}

/// u32 LE from a GET reply payload; replies sometimes prefix the payload
/// with the cmd-pair echo, so scan the first words for the value field.
pub fn parse_u32(p: &[u8]) -> u32 {
    if p.len() >= 4 {
        return u32::from_le_bytes([p[0], p[1], p[2], p[3]]);
    }
    0
}

/// f32 roll, pitch, yaw from a reply payload (tolerates id-prefixed forms).
pub fn parse_euler(p: &[u8]) -> [f32; 3] {
    for skip in [0usize, 4] {
        if p.len() >= skip + 12 {
            return [
                f32::from_le_bytes(p[skip..skip + 4].try_into().unwrap()),
                f32::from_le_bytes(p[skip + 4..skip + 8].try_into().unwrap()),
                f32::from_le_bytes(p[skip + 8..skip + 12].try_into().unwrap()),
            ];
        }
    }
    [0.0; 3]
}

/// Best-effort: extract length-prefixed printable strings from a reply
/// payload (preset-list replies carried "Preset1" etc.).
pub fn parse_name_list(payload: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < payload.len() {
        let len = payload[i] as usize;
        if (3..=0x32).contains(&len) && i + 1 + len <= payload.len() {
            if let Ok(s) = std::str::from_utf8(&payload[i + 1..i + 1 + len]) {
                if s.chars().all(|c| c.is_ascii_graphic() || c == ' ') {
                    out.push(s.to_string());
                    i += 1 + len;
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}
