//! Vendor command layer: wire cmd pair + flags + args packers.
//!
//! The USB/XU wire header is fully characterized in `crate::frame`
//! (capture-verified 2026-11-07): `aa 01 seq 00 0c 00 ck ck 0a flags c0 c1
//! args…` zero-padded to 60. Command semantics ride in the 2-byte cmd pair
//! at bytes 10..11 plus the args that follow; replies echo the cmd pair.
//!
//! The `Req` struct keeps the human-readable command catalog from the
//! static-RE phase. On this firmware build the wire cmd pair is NOT the
//! `(set<<12|id)` remap word assumed by the early static model — capture
//! shows small direct pairs (`08 18`, `04 3B`, `42 C4`…). `cmd_pair()`
//! returns the pair used on the wire; per-command mappings are pinned via
//! `.pair([...])` as each command is capture-verified, defaulting to
//! `[set, id as u8]` until then.

/// Protocol cmd_set values (as they appear in the decompile's cmd words).
pub mod set {
    pub const BASIC: u8 = 1; // camera basic (sleep, zoom stop, boot status)
    pub const GIMBAL: u8 = 2; // raw gimbal control
    pub const AI: u8 = 3; // AI / gimbal-with-tracking control
}

/// Frame-kind selectors observed in the decompile (kept for reference;
/// on the XU transport the kind does not appear as a distinct wire byte).
pub mod kind {
    pub const GET: u16 = 0x0C;
    pub const SET_SIMPLE: u16 = 0x0D;
    pub const SET: u16 = 0x0E;
    pub const SET_ACK: u16 = 0x10;
    pub const GIMBAL_SPEED_RST: u16 = 0x12;
    pub const GET_ARGS: u16 = 0x13;
    pub const TRACK_MODE: u16 = 0x14;
    pub const POS_CTRL: u16 = 0x18;
    pub const SPEED_CTRL_AI: u16 = 0x19;
    pub const PRESET_TRIGGER: u16 = 0x20;
}

/// Vendor cmd_ids, grouped by set (catalog from the decompile phase).
pub mod id {
    // set 1 (basic)
    pub const RUN_STATUS: u16 = 0x02;
    pub const BOOT_STATUS: u16 = 0x17;
    pub const ZOOM_STOP: u16 = 0x34;
    pub const TALLY_LIGHT: u16 = 0xA0; // cameraSetTallyLightR(bool)
    pub const TALLY_BRIGHT: u16 = 0xA1; // set: cameraSetTallyLightBrightnessR(int)
    //                        get: cameraGetTallyAndBatteryLightR(bool&,int&,bool&)
    //                             — same wire pair, pb args {2,4,5}
    pub const AUTO_SLEEP: u16 = 0x91;
    // set 2 (gimbal)
    pub const ATTITUDE: u16 = 0x00;
    pub const RST_POS_SN: u16 = 0x01;
    pub const SPEED: u16 = 0x02;
    pub const SPEED_POS_P2: u16 = 0x03;
    pub const SPEED_POS: u16 = 0x04;
    pub const RST_POS: u16 = 0x06;
    pub const INFO: u16 = 0x08;
    // set 3 (AI)
    pub const EULER: u16 = 0x04;
    pub const STATE: u16 = 0x0B;
    pub const STOP: u16 = 0x0C;
    pub const TRACK_ENABLE: u16 = 0x0D;
    pub const TRACK_MODE: u16 = 0x11;
    pub const PRESET_LIST: u16 = 0x35;
    pub const PRESET_LIST_2: u16 = 0x36; // products 4,B,16,10,12,13
    pub const PRESET_ADD: u16 = 0x37;
    pub const PRESET_GOTO: u16 = 0x3E;
    pub const PRESET_NAME: u16 = 0x5B;
    pub const TRACK_ENABLE_SN: u16 = 0x62;
    pub const NO_TRACK: u16 = 0x6C;
    pub const TRACK_TYPE: u16 = 0x71;
    pub const AUTO_GROUP: u16 = 0x72;
}

/// A vendor request: wire cmd pair + flags + args, with catalog metadata
/// kept for readability.
#[derive(Debug, Clone)]
pub struct Req {
pub kind: u16,
pub chan: u8,
pub cmd_set: u8,
pub cmd_id: u16,
/// Wire cmd pair (frame bytes 10..11). `None` => default below.
pub pair: Option<[u8; 2]>,
/// Frame type byte (frame byte 1). Capture: plain commands `0x01`,
/// preset-name reads `0x21`, `cd 10` query `0x25`. `None` => `0x01`.
pub ftype: Option<u8>,
/// Header flags byte (frame byte 9); the device does not validate it,
/// captures show 0x0D / 0x02 / 0x04 / 0x0A depending on family.
pub flags: u8,
pub args: Vec<u8>,
}

impl Req {
pub fn new(kind: u16, chan: u8, cmd_set: u8, cmd_id: u16) -> Self {
    Self {
        kind,
        chan,
        cmd_set,
        cmd_id,
        pair: None,
        ftype: None,
        flags: 0x0D,
        args: Vec::new(),
    }
}

pub fn args(mut self, a: impl Into<Vec<u8>>) -> Self {
    self.args = a.into();
    self
}

pub fn flags(mut self, f: u8) -> Self {
    self.flags = f;
    self
}

/// Pin the capture-verified frame type byte.
pub fn ftype(mut self, t: u8) -> Self {
    self.ftype = Some(t);
    self
}

    /// Pin the capture-verified wire cmd pair.
    pub fn pair(mut self, p: [u8; 2]) -> Self {
        self.pair = Some(p);
        self
    }

    /// 2-byte wire cmd pair for frame bytes 10..11.
    pub fn cmd_pair(&self) -> [u8; 2] {
        self.pair.unwrap_or([self.cmd_set, self.cmd_id as u8])
    }

    /// `(cmd_set << 16) | cmd_id` — catalog word (diagnostics only).
    pub fn cmd_word(&self) -> u32 {
        ((self.cmd_set as u32) << 16) | self.cmd_id as u32
    }

    /// Request args (the wire payload after the 12-byte header).
    pub fn to_payload(&self) -> &[u8] {
        &self.args
    }
}

fn f32le(x: f32) -> [u8; 4] {
    x.to_le_bytes()
}

fn i16_100(x: f32) -> [u8; 2] {
    ((x * 100.0).round() as i16).to_le_bytes()
}

// ---- constructors mirroring the decompiled exports -------------------------

/// `gimbalSpeedCtrlR` — signed speed per axis, -1.0..1.0 (→ -100..100).
/// Wire pair `42 C4` flags 0x02 — capture-verified (SET_CUR accepted,
/// `aa 29` reply echoed the pair; live replay 2026-11-07).
pub fn gimbal_speed(pan: f32, tilt: f32, roll: f32) -> Req {
    let mut a = Vec::with_capacity(6);
    a.extend_from_slice(&i16_100(pan));
    a.extend_from_slice(&i16_100(tilt));
    a.extend_from_slice(&i16_100(roll));
    Req::new(kind::GIMBAL_SPEED_RST, 2, set::GIMBAL, id::SPEED)
        .pair([0x42, 0xC4])
        .flags(0x02)
        .args(a)
}

/// `gimbalRstPosR` (SN mode) — reset gimbal position.
pub fn gimbal_rst_pos(mode: u8) -> Req {
    Req::new(kind::GIMBAL_SPEED_RST, 2, set::GIMBAL, id::RST_POS_SN).args(vec![mode, 0, 0])
}

/// `gimbalGetAttitudeInfoR` — get attitude/status
/// (reply payload: f32 roll, pitch, yaw + status). Wire pair `08 18`
/// flags 0x0D — capture-verified (first captured SET_CUR frame); live
/// reply payload = 24 B: three f32 zeros at camera level.
pub fn gimbal_attitude() -> Req {
    Req::new(kind::GET, 2, set::GIMBAL, id::ATTITUDE).pair([0x08, 0x18])
}

/// Serial-number read — reply payload (14 B, live-verified 2026-11-07) is
/// the ASCII serial, e.g. `RMOWLHGB081JIL`. Wire pair `C8 18` flags 0x0D.
pub fn gimbal_serial() -> Req {
    Req::new(kind::GET, 2, set::GIMBAL, id::INFO)
        .pair([0xC8, 0x18])
        .flags(0x0D)
}

/// `aiGetGimbalPresetListR` — read ONE preset name by index. Wire form
/// capture-verified: frame type `0x21`, flags `0x04`, args
/// `[04 00 ck0 ck1 idx_lo idx_hi]` where `ck` = `frame::sub_ck` over the
/// 4-byte payload `[idx 0 0 0]` (device rejects wrong checksums:
/// probe `sub=00 00` → no reply). Reply payload = the ASCII name
/// ("Preset1", "P2", "P3" live-verified 2026-11-07).
pub fn preset_name(index: u8) -> Req {
    let sub = crate::frame::sub_ck(&[index, 0, 0, 0]);
    Req::new(kind::GET, 3, set::AI, id::PRESET_LIST)
        .pair([0x04, 0x3B])
        .ftype(0x21)
        .flags(0x04)
        .args(vec![0x04, 0x00, sub[0], sub[1], index, 0])
}

/// `cameraSetAiModeU` — AI framing mode. NOTE: on the wire this rides XU
/// selector 6 as a raw struct, not an AA frame (see `xuprobe` example).
pub fn ai_mode(mode: u8) -> Req {
    Req::new(kind::TRACK_MODE, 3, set::AI, id::TRACK_MODE).args(vec![mode, 0, 0, 0])
}

/// `aiSetGimbalEulerAngleR` — absolute pan/tilt/roll in degrees.
pub fn gimbal_euler(yaw: f32, pitch: f32, roll: f32) -> Req {
    let mut a = Vec::with_capacity(12);
    a.extend_from_slice(&f32le(yaw));
    a.extend_from_slice(&f32le(pitch));
    a.extend_from_slice(&f32le(roll));
    Req::new(kind::POS_CTRL, 3, set::AI, id::EULER).args(a)
}

/// `aiSetGimbalStop` — stop all gimbal motion.
pub fn gimbal_stop() -> Req {
    Req::new(kind::GET, 3, set::AI, id::STOP)
}

/// `aiGetGimbalStateR` — get `AiGimbalStateInfo`.
pub fn gimbal_state() -> Req {
    Req::new(kind::GET, 3, set::AI, id::STATE)
}

/// `aiSetEnabledR` — AI tracking on/off (no-SN path).
/// Word `0x3000D` → pair `0D 30`. The unpinned default `[set, id]` is
/// `03 0D`, which the camera echoes and then ignores.
pub fn tracking_enable(on: bool) -> Req {
    Req::new(kind::SET, 3, set::AI, id::TRACK_ENABLE)
        .pair([0x0D, 0x30])
        .args(vec![on as u8])
}

/// `aiSetTrackingModeR` — `AiVerticalTrackType` (0 = full, other modes per SDK).
/// Word `0x30011` → pair `11 30`.
pub fn tracking_mode(mode: u8) -> Req {
    Req::new(kind::SET, 3, set::AI, id::TRACK_MODE)
        .pair([0x11, 0x30])
        .args(vec![mode])
}

/// `aiSetAiTrackModeEnabledR` — target-selection type table from decompile.
/// Human and the other ids ride pair `71 30` with two u32s (category, sub).
/// Auto-group (`0x1E`) rides pair `72 30` with those two u32s zero.
/// The two-u32 layout is the decompile; only the wire pair was unpinned.
pub fn tracking_type(mode: u16) -> Req {
    let (pair, sel0, sel1) = match mode {
        1 => ([0x71, 0x30], 0u32, 0u32),
        2 => ([0x71, 0x30], 0, 4),
        3 => ([0x71, 0x30], 0, 3),
        4 => ([0x71, 0x30], 0, 2),
        5 => ([0x71, 0x30], 0, 1),
        0xA => ([0x71, 0x30], 1, 0),
        0xB => ([0x71, 0x30], 1, 2),
        0xC => ([0x71, 0x30], 1, 1),
        _ => ([0x72, 0x30], 0, 0), // 0x1E auto-group
    };
    let mut a = Vec::with_capacity(8);
    a.extend_from_slice(&sel0.to_le_bytes());
    a.extend_from_slice(&sel1.to_le_bytes());
    let id = if pair == [0x72, 0x30] {
        id::AUTO_GROUP
    } else {
        id::TRACK_TYPE
    };
    Req::new(kind::TRACK_MODE, 3, set::AI, id)
        .pair(pair)
        .args(a)
}

/// Turn a track target off. Human and the other ids use
/// `aiDelSelectedTargetR` (`70 30`). Auto-group uses
/// `aiDelAutoGroupModeR` (`73 30`). Neither carries args.
pub fn tracking_clear(mode: u16) -> Req {
    if mode == 0x1E {
        Req::new(kind::GET, 3, set::AI, 0x73).pair([0x73, 0x30])
    } else {
        Req::new(kind::GET, 3, set::AI, 0x70).pair([0x70, 0x30])
    }
}

/// `aiTrgGimbalPresetR` — go to a saved gimbal preset by id.
pub fn preset_goto(preset_id: u32) -> Req {
    Req::new(kind::PRESET_TRIGGER, 3, set::AI, id::PRESET_GOTO).args(preset_id.to_le_bytes())
}

/// `aiAddGimbalPresetR` — id + position (deg) + zoom + name.
pub fn preset_add(preset_id: u32, x: f32, y: f32, z: f32, zoom: f32, name: &[u8]) -> Req {
    let mut a = Vec::with_capacity(4 + 6 + 4 + 1 + name.len());
    a.extend_from_slice(&preset_id.to_le_bytes());
    a.extend_from_slice(&i16_100(x));
    a.extend_from_slice(&i16_100(y));
    a.extend_from_slice(&i16_100(z));
    a.extend_from_slice(&f32le(zoom));
    let n = name.len().min(0x32);
    a.push(n as u8);
    a.extend_from_slice(&name[..n]);
    Req::new(kind::GET, 3, set::AI, id::PRESET_ADD).args(a)
}

/// `cameraSetAutoSleepTimeR` — auto-sleep timer in seconds. `0` is off.
/// libdev @ 0xfa0d0 writes the seconds as a u32 and the cmd word `0x10091`,
/// so the wire pair is `[0x91, 0x10]`.
pub fn auto_sleep(seconds: u32) -> Req {
    Req::new(kind::SET_ACK, 1, set::BASIC, id::AUTO_SLEEP)
        .pair([0x91, 0x10])
        .args(seconds.to_le_bytes())
}

/// `DevStatus` value Center passes to wake the camera. Connect paths always
/// pass 1. The Sleep button passes 3 on this product (not 0x16).
pub const RUN_AWAKE: u8 = 1;
pub const RUN_SLEEP: u8 = 3;

/// `cameraSetDevRunStatusR` — libdev @ 0x2e480. With no serial word the
/// payload is the byte `0x01` then the status. Pair matches the other
/// set-1 commands (`[cmd_id, 0x10]`).
pub fn run_status(status: u8) -> Req {
    Req::new(kind::SET, 1, set::BASIC, id::RUN_STATUS)
        .pair([0x02, 0x10])
        .args(vec![0x01, status])
}

/// `cameraGetBootStatus` — boot/ready probe.
/// Live-verified 2026-11-07: wire pair `17 10` (id 0x17, setcode 1 nibble),
/// type 0x01, flags 0x0D, no args. The device echoes the pair in a type-09
/// status-ack; Tiny 2 Lite firmware supplies no dedicated boot payload
/// (any bytes past the header are stale async-event buffer residue), so an
/// ack = camera present and responsive.
pub fn boot_status() -> Req {
    Req::new(kind::GET, 1, set::BASIC, id::BOOT_STATUS).pair([0x17, 0x10])
}

/// `cameraSetZoomStopR` — stop vendor zoom (absolute/relative zoom is UVC).
pub fn zoom_stop() -> Req {
    Req::new(kind::GET, 1, set::BASIC, id::ZOOM_STOP)
}

/// `cameraSetTallyLightR(bool)` — libdev@0xF1DE0 (REA): cmd word
/// `0x100A0` → wire pair `[0xA0, 0x10]`, pb arg **field 3** varint (0/1).
/// libdev sends it without waiting for a reply; the device still status-
/// ACKs the pair like other sets.
pub fn tally_light(on: bool) -> Req {
    Req::new(kind::GET, 1, set::BASIC, id::TALLY_LIGHT)
        .pair([0xA0, 0x10])
        .args([0x18, on as u8]) // tag = field3<<3|varint
}

/// `cameraSetTallyLightBrightnessR(int)` — libdev@0xF1F40 (REA): **same**
/// cmd word `0x100A0` as tally_light — pair `[0xA0, 0x10]` — the two sets
/// are distinguished by pb field: brightness is **field 4** varint.
pub fn tally_brightness(level: u32) -> Req {
    let mut a = vec![0x20u8]; // tag = field4<<3|varint
    let mut v = level;
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        a.push(if v != 0 { b | 0x80 } else { b });
        if v == 0 {
            break;
        }
    }
    Req::new(kind::GET, 1, set::BASIC, id::TALLY_LIGHT)
        .pair([0xA0, 0x10])
        .args(a)
}

/// `cameraGetTallyAndBatteryLightR` — libdev@0xF2200 (REA): cmd word
/// `0x100A1` → pair `[0xA1, 0x10]`, pb request fields {2,4,5} (all zero
/// varints). CAVEAT: libdev short-circuits this GET for model 1/2 devices
/// (reads a cache kept fresh by async 0x89 status events). The Tiny 2
/// Lite's model id falls in that class, so the device may legitimately
/// never answer on the wire — treat a missing reply as "unknown state",
/// not as a transport fault.
pub fn get_tally() -> Req {
    Req::new(kind::GET, 1, set::BASIC, id::TALLY_BRIGHT)
        .pair([0xA1, 0x10])
        .args([0x10, 0x00, 0x20, 0x00, 0x28, 0x00])
}

/// Decode a `get_tally` reply payload: pb fields 1=tally_light(bool),
/// 2=brightness(int), 3=battery_light(bool) → `(tally, bright, battery)`.
/// Missing fields keep the default passed in.
pub fn parse_tally(p: &[u8], tally: bool, bright: u32, battery: bool) -> (bool, u32, bool) {
    let (mut t, mut b, mut bat) = (tally, bright, battery);
    let mut i = 0;
    while i < p.len() {
        let tag = p[i];
        i += 1;
        let field = tag >> 3;
        match tag & 7 {
            0 => {
                // varint
                let mut v: u32 = 0;
                let mut sh = 0;
                while i < p.len() {
                    let byte = p[i];
                    i += 1;
                    v |= ((byte & 0x7f) as u32) << sh;
                    sh += 7;
                    if byte & 0x80 == 0 {
                        break;
                    }
                }
                match field {
                    1 => t = v == 1,
                    2 => b = v,
                    3 => bat = v == 1,
                    _ => {}
                }
            }
            2 => {
                if i + 4 <= p.len() {
                    let v = u32::from_le_bytes([p[i], p[i + 1], p[i + 2], p[i + 3]]);
                    match field {
                        1 => t = v == 1,
                        2 => b = v,
                        3 => bat = v == 1,
                        _ => {}
                    }
                    i += 4;
                } else {
                    break;
                }
            }
            _ => break,
        }
    }
    (t, b, bat)
}

// Wire pair for command word 0xS_ID is [ID, S<<4]. Setter args are a
// little-endian u32. This firmware uses the no-serial shape for set 1
// (sleep and wake), so these ride the same status-ACK pattern as tally:
// pinned pair, then a kind::GET request.

/// `cameraSetFaceFocusR` @0x18002f6e0 — vendor face-focus flag, internal
/// word `0x10003` -> pair `03 10`, arg u32 bool. Rides ON TOP of the UVC
/// auto-focus switch (CID_FOCUS_AUTO untouched by libdev here).
pub fn face_focus(on: bool) -> Req {
    Req::new(kind::GET, 1, set::BASIC, 0x03)
        .pair([0x03, 0x10])
        .args((on as u32).to_le_bytes().to_vec())
}

/// `cameraSetAutoFocusModeR` @0x1800f69b0 — DevAutoFocusType, internal
/// `0x1005f` -> pair `5F 10`, arg u32 mode (0..N settle by live readback).
pub fn set_af_mode(mode: u32) -> Req {
    Req::new(kind::GET, 1, set::BASIC, 0x5F)
        .pair([0x5F, 0x10])
        .args(mode.to_le_bytes().to_vec())
}

/// `cameraGetAutoFocusModeR` @0x1800f6a50 — internal `0x1005e` -> pair
/// `5E 10`, GET; reply payload u32 = current mode.
pub fn get_af_mode() -> Req {
    Req::new(kind::GET, 1, set::BASIC, 0x5E).pair([0x5E, 0x10])
}

/// `cameraSetExposureModeR` @0x1800f3aa0 — internal `0x10047` -> pair
/// `47 10`, arg u32 mode.
pub fn set_exp_mode(mode: u32) -> Req {
    Req::new(kind::GET, 1, set::BASIC, 0x47)
        .pair([0x47, 0x10])
        .args(mode.to_le_bytes().to_vec())
}

/// `cameraGetExposureModeR` @0x1800f3b40 — internal `0x10046` -> pair
/// `46 10`, GET; reply u32 = mode.
pub fn get_exp_mode() -> Req {
    Req::new(kind::GET, 1, set::BASIC, 0x46).pair([0x46, 0x10])
}

/// `cameraSetWdrR` @0x18002f180 AA fallback — internal `0x10014` -> pair
/// `14 10`, arg u32 (0/1). Only used when product ∈ {4,B,16,10}; on the
/// Lite HDR rides `ai_field(0x0101)` on selector 6 instead.
pub fn wdr_aa(on: bool) -> Req {
    Req::new(kind::GET, 1, set::BASIC, 0x14)
        .pair([0x14, 0x10])
        .args((on as u32).to_le_bytes().to_vec())
}

/// `cameraSetMirrorFlipR` @0x180031600 — internal `0x1002e` -> pair
/// `2E 10`, arg u32 value (mirror/flip combo).
///
/// The camera status-acks this pair. The UVC node has no hflip/vflip
/// control, and cycling 0..3 does not change the MJPEG preview, so the
/// value is not applied on the stream this panel displays.
pub fn set_mirror_flip(v: u32) -> Req {
    Req::new(kind::GET, 1, set::BASIC, 0x2E)
        .pair([0x2E, 0x10])
        .args(v.to_le_bytes().to_vec())
}

/// `cameraGetMirrorFlipR` @0x1800316a0 — internal `0x1002d` -> pair
/// `2D 10`, GET; reply u32.
pub fn get_mirror_flip() -> Req {
    Req::new(kind::GET, 1, set::BASIC, 0x2D).pair([0x2D, 0x10])
}

/// `aiSetCameraZoomRatioR` @0x18001dec0 — kind `0x14` (AI ratio family),
/// internal `0x30034` -> pair `34 30`, args f32 ratio + i32. AI auto-zoom
/// path; manual zoom on the Lite is the UVC `zoom_absolute` CID (verified
/// against cameraSetZoomAbsoluteR decompile: Center writes the UVC ctrl).
/// `aiSetHandTrackModeR` @0x18001b570 — pair `49 30`, one byte.
/// `AiHandTrackType` names were not in libdev; the UI sends 0..N.
pub fn hand_track_mode(mode: u8) -> Req {
    Req::new(kind::GET, 3, set::AI, 0x49)
        .pair([0x49, 0x30])
        .args(vec![mode])
}

/// `aiSetHandTrackGimbalEnabledR` @0x18001bad0 — pair `57 30`, bool byte.
pub fn hand_track_gimbal(on: bool) -> Req {
    Req::new(kind::GET, 3, set::AI, 0x57)
        .pair([0x57, 0x30])
        .args(vec![on as u8])
}

/// `aiSetGimbalYawDirReverseR` @0x180020220 — pair `65 30` on product 3.
/// Yaw only. No tilt-reverse command exists.
pub fn yaw_reverse(on: bool) -> Req {
    Req::new(kind::GET, 3, set::AI, 0x65)
        .pair([0x65, 0x30])
        .args(vec![on as u8])
}

/// `aiSetTrackSpeedTypeR` @0x180021220 — pair `68 30`, one byte.
/// 0/1/2 are the slow/normal/fast candidates; names are not in the DLL.
pub fn track_speed(level: u8) -> Req {
    Req::new(kind::GET, 3, set::AI, 0x68)
        .pair([0x68, 0x30])
        .args(vec![level])
}

/// `aiSetHorizontalOffset` @0x1800209e0 — pair `79 30`, f32 LE.
pub fn horizontal_offset(value: f32) -> Req {
    Req::new(kind::GET, 3, set::AI, 0x79)
        .pair([0x79, 0x30])
        .args(value.to_le_bytes().to_vec())
}

/// `aiSetVerticalOffset` @0x180020b50 — pair `7B 30`, f32 LE.
pub fn vertical_offset(value: f32) -> Req {
    Req::new(kind::GET, 3, set::AI, 0x7B)
        .pair([0x7B, 0x30])
        .args(value.to_le_bytes().to_vec())
}

/// Volume set @0x1800f7ea0 — pair `74 10`, one byte 0–100.
pub fn audio_volume(level: u8) -> Req {
    Req::new(kind::GET, 1, set::BASIC, 0x74)
        .pair([0x74, 0x10])
        .args(vec![level.min(100)])
}

/// `aiDelGimbalPresetR` @0x18001F860 — pair `38 30`, u32 id first.
pub fn preset_delete(id: u32) -> Req {
    Req::new(kind::GET, 3, set::AI, 0x38)
        .pair([0x38, 0x30])
        .args(id.to_le_bytes().to_vec())
}

/// `aiSetGimbalPresetNameWithIdR` @0x18001F210 — pair `3D 30`.
/// u32 id then a NUL-terminated UTF-8 name, at most 47 bytes.
pub fn preset_rename(id: u32, name: &str) -> Req {
    let mut args = id.to_le_bytes().to_vec();
    let raw = name.as_bytes();
    let n = raw.len().min(47);
    args.extend_from_slice(&raw[..n]);
    args.push(0);
    Req::new(kind::GET, 3, set::AI, 0x3D)
        .pair([0x3D, 0x30])
        .args(args)
}

/// `aiAddGimbalPresetR` @0x18001F380 — pair `37 30`, 24 bytes.
/// Product-3 pack is six little-endian f32s: id, then the pose.
/// Axis order inside the pose is still unverified.
pub fn preset_add_lite(id: u32, yaw_deg: f32, pitch_deg: f32, zoom: f32) -> Req {
    let mut args = Vec::with_capacity(24);
    for value in [id as f32, yaw_deg, pitch_deg, 0.0, zoom, 0.0] {
        args.extend_from_slice(&value.to_le_bytes());
    }
    Req::new(kind::GET, 3, set::AI, 0x37)
        .pair([0x37, 0x30])
        .args(args)
}

/// `aiGetAiStatusR` @0x180019280 — GET pair `13 30`.
/// Async `0x89` events are not visible through the kernel XU path.
pub fn get_ai_status() -> Req {
    Req::new(kind::GET, 3, set::AI, 0x13).pair([0x13, 0x30])
}

/// Selector-6 field `0x0111` stores **disabled**, not enabled.
pub fn bg_disabled_byte(background_on: bool) -> u8 {
    u8::from(!background_on)
}

/// Selector-6 field `0x011C` stores **enabled** (`!disabled`).
pub fn mic_enabled_byte(mic_disabled: bool) -> u8 {
    u8::from(!mic_disabled)
}

pub fn ai_zoom_ratio(ratio: f32, speed: i32) -> Req {
    let mut a = ratio.to_le_bytes().to_vec();
    a.extend_from_slice(&speed.to_le_bytes());
    Req::new(kind::TRACK_MODE, 3, set::AI, 0x34)
        .pair([0x34, 0x30])
        .args(a)
}
