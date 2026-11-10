// OBSBOT device control panel — the Rust rebuild of the OBSBOT_Main camera
// panel (gimbal, AI tracking, presets, zoom, tally LED, live video, status).
// Preview capture is 30 fps. 60 fps on usbip drops the camera when a
// selector-6 control is sent. The window still repaints on a 1/60 clock.
//
// Kernel path: XU control frames via UVCIOC_CTRL_QUERY on /dev/video1,
// MJPEG frames via V4L2 mmap on /dev/video0, zoom via VIDIOC_S_CTRL.
// Run inside WSLg (it shows a window):  sudo cargo run --features panel --example panel
// Headless smoke test (no GUI):         sudo cargo run --features panel --example panel -- --shots=5
#![allow(dead_code)]

use eframe::egui;
use obsbot_core::device::ObsbotUsb;
use obsbot_core::linux_kernel::{
    Capture, CtrlNode, CtrlRange, V4lXu, CID_AUTO_WHITE_BALANCE, CID_BACKLIGHT_COMPENSATION,
    CID_CONTRAST, CID_EXPOSURE_AUTO, CID_FOCUS_AUTO, CID_HUE, CID_PAN_ABSOLUTE,
    CID_POWER_LINE_FREQUENCY, CID_SATURATION, CID_SHARPNESS, CID_TILT_ABSOLUTE,
    CID_WHITE_BALANCE_TEMPERATURE, CID_ZOOM_ABSOLUTE,
};
use obsbot_core::cmd;
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Preview present clock. eframe adds `request_repaint_after`'s delay on top
/// of `SwapInterval::Wait`, so a 16 ms delay with vsync on presents every
/// other vblank (~30 fps). Vsync is off and this grid is the only pace.
const PLAYBACK_HZ: u64 = 60;
const PLAYBACK_STEP_NS: u64 = 1_000_000_000 / PLAYBACK_HZ;

/// Tiny 2 Lite is product type 4. `gimbalRstPosR` mode is 0 here;
/// 0x7D is only for products B / 0x16 / 0x10.
const TINY2_LITE_RESET_MODE: u8 = 0;

/// Live tally probe uses 100 as the bright end (`probe_tally`).
const TALLY_BRIGHT_MAX: u32 = 100;

/// Hold-to-move rate at speed 1.0. UVC pan/tilt absolute is in arcseconds
/// (3600 = 1°). `pan_speed` / `tilt_speed` accept writes and do not move
/// this gimbal; `pan_absolute` does.
const GIMBAL_MAX_DEG_PER_SEC: f32 = 45.0;
const ARCSEC_PER_DEG: f32 = 3600.0;

/// Center's manual-zoom readout runs 1.00× at the bottom of `zoom_absolute`
/// to 4.00× at the top. The ioctl still gets the raw control value.
/// Zoom ratio label, byte-for-byte the formula libdev.dll
/// `Device::cameraGetZoomAbsoluteR` runs for this product (product type 3,
/// `Tail2Lite`): `1 + (cur - min) / (max - min)` off the V4L2 zoom_absolute
/// ctrl. The x3.0 branch in libdev belongs to the AA-pair products only;
/// Center shows 1.00x-2.00x on this camera because its UVC zoom range is
/// 0-100 with this normalization.
fn zoom_times(value: i32, min: i32, max: i32) -> f32 {
    let span = (max - min).max(1) as f32;
    1.0 + (value.clamp(min, max) - min) as f32 / span
}

const BG: egui::Color32 = egui::Color32::from_rgb(18, 18, 20);
const PANEL: egui::Color32 = egui::Color32::from_rgb(28, 28, 31);
const CARD: egui::Color32 = egui::Color32::from_rgb(44, 44, 48);
const RED: egui::Color32 = egui::Color32::from_rgb(255, 59, 92);
const TEXT: egui::Color32 = egui::Color32::from_rgb(236, 236, 240);
const MUTED: egui::Color32 = egui::Color32::from_rgb(154, 154, 162);

/// UVC controls that line up with Center's Image page. A control the camera
/// does not expose is left out rather than shown at a fake value.
const IMAGE_CTRLS: &[(u32, &str)] = &[
    (CID_FOCUS_AUTO, "Auto Focus"),
    (CID_EXPOSURE_AUTO, "Auto Exposure"),
    (CID_BACKLIGHT_COMPENSATION, "Compensation"),
    (CID_POWER_LINE_FREQUENCY, "Anti-Flicker"),
    (CID_AUTO_WHITE_BALANCE, "Auto WB"),
    (CID_WHITE_BALANCE_TEMPERATURE, "Temperature"),
    (CID_CONTRAST, "Contrast"),
    (CID_SATURATION, "Saturation"),
    (CID_SHARPNESS, "Sharpness"),
    (CID_HUE, "Hue"),
];

/// `AiVerticalTrackType`. The decompile notes 0 as full-body.
const FRAMING_MODES: &[(u8, &str)] = &[
    (0, "Full"),
    (1, "1"),
    (2, "2"),
    (3, "3"),
    (4, "4"),
];

/// Delay until the next 60 Hz boundary. Never zero, so a frame that lands
/// exactly on a boundary waits one whole step instead of spinning.
fn playback_delay(elapsed_ns: u64) -> Duration {
    let next = (elapsed_ns / PLAYBACK_STEP_NS + 1) * PLAYBACK_STEP_NS;
    Duration::from_nanos(next - elapsed_ns)
}

/// Move one axis by at most a single hardware step.
///
/// The Tiny 2 Lite rounds pan/tilt to 1° (3600 arcsec). Applying a whole
/// frame of motion at once jumps several degrees. Leftover arcseconds stay
/// in `frac` and come out on later ticks, so the motor steps at an even rate.
fn step_axis(
    pos: i32,
    frac: f32,
    deg_per_sec: f32,
    dt: f32,
    step: i32,
    min: i32,
    max: i32,
) -> (i32, f32) {
    if step <= 0 || deg_per_sec == 0.0 || dt <= 0.0 {
        return (pos.clamp(min, max), frac);
    }
    let mut acc = frac + deg_per_sec * dt * ARCSEC_PER_DEG;
    let mut pos = pos;
    let step_f = step as f32;
    if acc >= step_f {
        acc -= step_f;
        pos += step;
    } else if acc <= -step_f {
        acc += step_f;
        pos -= step;
    }
    let clamped = pos.clamp(min, max);
    if clamped != pos {
        acc = 0.0;
    }
    (clamped, acc)
}

/// Unit-length stick times `speed`, so a diagonal is not faster than a cardinal.
fn scaled_stick(dx: f32, dy: f32, speed: f32) -> (f32, f32) {
    let mag = (dx * dx + dy * dy).sqrt();
    if mag == 0.0 {
        return (0.0, 0.0);
    }
    (dx / mag * speed, dy / mag * speed)
}

/// Pad yaw. `65 30` acks and does not invert the absolute-pan steps this
/// stick sends, so the reverse switch flips the pan sign here. Tilt is not
/// reversed; libdev has no tilt-reverse command.
fn stick_axes(dx: f32, dy: f32, speed: f32, yaw_reversed: bool) -> (f32, f32) {
    let (pan, tilt) = scaled_stick(dx, dy, speed);
    if yaw_reversed { (-pan, tilt) } else { (pan, tilt) }
}

/// One Hand click was delivered twice and the second one turned tracking
/// off before the camera had finished the first command.
const HAND_GESTURE_MS: u128 = 500;

/// `Some(true)` turns hand tracking on, `Some(false)` turns it off.
/// `None` is the echo of the click that just turned it on.
fn hand_gesture(on: bool, elapsed_ms: u128) -> Option<bool> {
    if on && elapsed_ms < HAND_GESTURE_MS {
        None
    } else {
        Some(!on)
    }
}

/// `Some(new)` when the user picked a different option.
fn changed_pick<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    id: &str,
    current: Option<T>,
    options: &[(T, &str)],
) -> Option<T> {
    let text = current
        .and_then(|c| options.iter().find(|(v, _)| *v == c).map(|(_, name)| *name))
        .unwrap_or("—");
    let mut slot = current;
    egui::ComboBox::from_id_salt(id)
        .selected_text(text)
        .show_ui(ui, |ui| {
            for &(value, name) in options {
                ui.selectable_value(&mut slot, Some(value), name);
            }
        });
    if slot != current { slot } else { None }
}

// ---- messages ---------------------------------------------------------------

enum UiMsg {
    Speed(f32, f32),
    Stop,
    Rst,
    Track(bool),
    TrackMode(u8),
    TrackType(u16),
    /// `70 30` (selected target) or `73 30` (auto-group).
    TrackClear(u16),
    /// `cameraSetAiModeU` selector-6 field `0216`: work mode, then target type.
    /// Product-3 human is `(2, 1)`; off is `(0, 0)`.
    AiWork(u8, u8),
    Goto(u32),
    Tally(bool),
    Bright(u32),
    /// `cmd::RUN_AWAKE` or `cmd::RUN_SLEEP`.
    Run(u8),
    /// Auto-sleep timer in seconds. `0` turns it off.
    AutoSleep(u32),
    /// Vendor face-focus flag, pair `03 10` (rides on top of UVC auto-focus).
    FaceFocus(bool),
    /// DevAutoFocusType via pair `5F 10`; device settles the enum domain.
    AfMode(u32),
    /// Exposure mode via pair `47 10`.
    ExpMode(u32),
    /// HDR/WDR via cameraSetWdrR AA pair `14 10`.
    Hdr(bool),
    /// cameraSetMirrorFlipR value, pair `2E 10`.
    Mirror(u32),
    /// FaceAE via AiMode selector-6 field `0103`.
    FaceAe(bool),
    /// `aiSetHandTrackModeR` pair `49 30`.
    HandMode(u8),
    /// `aiSetHandTrackGimbalEnabledR` pair `57 30`.
    HandGimbal(bool),
    /// `aiSetGimbalYawDirReverseR` pair `65 30`.
    YawReverse(bool),
    /// `aiSetTrackSpeedTypeR` pair `68 30`.
    TrackSpeed(u8),
    /// Horizontal / vertical track offset, pairs `79 30` / `7B 30`.
    TrackOffset { horizontal: bool, value: f32 },
    /// Volume pair `74 10`, byte 0–100.
    Volume(u8),
    /// Selector-6 audio: AGC `0117`, noise `010A`, mic enable `011C`,
    /// distance `011B`, mic during sleep `0113`.
    AudioField { field: u16, value: u8 },
    /// Selector-6 sleep background: mode `0105`, color `0110`, enable `0111`.
    BgField { field: u16, value: u8 },
    /// `aiAddGimbalPresetR` pair `37 30`.
    PresetAdd { id: u32, yaw: f32, pitch: f32, zoom: f32 },
    /// `aiDelGimbalPresetR` pair `38 30`.
    PresetDel(u32),
    /// `aiSetGimbalPresetNameWithIdR` pair `3D 30`.
    PresetRename { id: u32, name: String },
}

enum Ev {
    Log(String),
    Serial(String),
    Boot(bool),
    Attitude([f32; 3]),
    Presets(Vec<String>),
    Frame(std::sync::Arc<Decoded>), // decoded preview + original jpeg
    CamFps(f32),
    Zoom(CtrlRange, i32),
    /// Current pan/tilt plus the V4L2 ranges (arcseconds).
    Ptz(CtrlRange, i32, CtrlRange, i32),
    Images(Vec<ImageCtrl>),
    /// One-line `aiGetAiStatusR` (`13 30`) summary.
    Status(String),
}

/// One UVC processing or camera-class control read at startup.
#[derive(Clone)]
struct ImageCtrl {
    id: u32,
    name: &'static str,
    range: CtrlRange,
    value: i32,
}

/// Commands for the control fd. The vendor XU speed frame does not move
/// the gimbal while uvcvideo is streaming, so the pad is a velocity and
/// the control thread turns it into absolute steps.
enum CapCmd {
    Zoom(i32),
    /// Pan and tilt velocity in degrees per second. Zero holds position.
    Stick(f32, f32),
    Home,
    /// A processing-unit or camera-class control (contrast, white balance, …).
    SetCtrl(u32, i32),
}

/// A decoded preview frame plus the original MJPEG payload (for saving).
struct Decoded {
    img: std::sync::Arc<egui::ColorImage>,
    raw: std::sync::Arc<Vec<u8>>,
}

/// Fast MJPEG decode: zune-jpeg first, jpeg-decoder as fallback. Height is
/// derived from the decoded pixel count — OBSBOT frames carry DNL-corrected
/// heights whose SOF marker header sometimes lies (e.g. 1280x1280 anomaly).
fn decode_frame(bytes: Vec<u8>) -> Option<Decoded> {
    let img = decode_zune(&bytes).or_else(|| decode_jpegstd(&bytes))?;
    Some(Decoded {
        img: std::sync::Arc::new(img),
        raw: std::sync::Arc::new(bytes),
    })
}

fn decode_zune(bytes: &[u8]) -> Option<egui::ColorImage> {
    use zune_jpeg::zune_core::colorspace::ColorSpace;
    let mut dec = zune_jpeg::JpegDecoder::new(bytes);
    dec.decode_headers().ok()?;
    let info = dec.info()?;
    let w = info.width as usize;
    if w == 0 {
        return None;
    }
    let bpp = match dec.get_output_colorspace() {
        Some(ColorSpace::CMYK) | Some(ColorSpace::YCCK) => 4,
        _ => 3, // RGB output for YCbCr (what OBSBOT MJPEG decodes to)
    };
    let samples = dec.decode().ok()?;
    if samples.len() % (w * bpp) != 0 {
        return None;
    }
    let h = samples.len() / (w * bpp);
    let mut rgba = Vec::with_capacity(w * h * 4);
    for s in samples.chunks(bpp) {
        let (r, g, b) = (s[0], s[1], s[2.min(bpp - 1)]);
        rgba.extend_from_slice(&[r, g, b, 255]);
    }
    Some(egui::ColorImage::from_rgba_unmultiplied(
        [w, h],
        &rgba,
    ))
}

fn decode_jpegstd(bytes: &[u8]) -> Option<egui::ColorImage> {
    let mut dec = jpeg_decoder::Decoder::new(bytes);
    let Ok(samples) = dec.decode() else {
        return None;
    };
    let Some(info) = dec.info() else {
        return None;
    };
    let w = info.width as usize;
    if w == 0 {
        return None;
    }
    let pb = info.pixel_format.pixel_bytes().max(1);
    if samples.len() % (w * pb) != 0 {
        return None;
    }
    let h = samples.len() / (w * pb);
    let mut rgba = Vec::with_capacity(w * h * 4);
    for s in samples.chunks(pb) {
        let (r, g, b) = if pb >= 3 {
            (s[0], s[1], s[2])
        } else {
            (s[0], s[0], s[0])
        };
        rgba.extend_from_slice(&[r, g, b, 255]);
    }
    Some(egui::ColorImage::from_rgba_unmultiplied(
        [w, h],
        &rgba,
    ))
}

/// Decode thread: drains to the newest raw frame before decoding, so the
/// UI always sees the freshest image at max achievable rate instead of a
/// lagging queue (the old design decoded on the UI thread -> ~10 fps).
fn spawn_decode(ev: Sender<Ev>, raw_rx: Receiver<Vec<u8>>) {
    std::thread::spawn(move || {
        let mut err_logged = false;
        loop {
            let Ok(mut bytes) = raw_rx.recv() else { break };
            while let Ok(n) = raw_rx.try_recv() {
                bytes = n;
            }
            match decode_frame(bytes) {
                Some(d) => {
                    if ev.send(Ev::Frame(std::sync::Arc::new(d))).is_err() {
                        break;
                    }
                }
                None => {
                    if !err_logged {
                        let _ = ev.send(Ev::Log("decode: no usable frame yet".into()));
                        err_logged = true;
                    }
                }
            }
        }
    });
}

// ---- background threads -----------------------------------------------------

/// Capture pauses the 60 fps stream while the control thread holds this.
/// `held` is true only after STREAMOFF, so a selector-6 write does not
/// start until the isochronous pipe is stopped.
struct StreamHold {
    pause: AtomicBool,
    held: AtomicBool,
}

impl StreamHold {
    fn new() -> Self {
        Self {
            pause: AtomicBool::new(false),
            held: AtomicBool::new(false),
        }
    }

    /// Stop capture, run `body`, then let capture STREAMON at 30 fps.
    ///
    /// A selector-6 SET on a live stream times the video pipe out
    /// (`-62` then `-32`) and usbip closes the socket. That happened at
    /// 60 fps and again at 30 fps (15:13:33). The write has to go out
    /// while the video transfers are cancelled. The stream stays down
    /// until those cancels drain, and it comes back at 30 fps.
    fn around(
        &self,
        body: impl FnOnce() -> Result<(), Box<dyn Error>>,
    ) -> Result<(), Box<dyn Error>> {
        struct Release<'a>(&'a StreamHold);
        impl Drop for Release<'_> {
            fn drop(&mut self) {
                self.0.pause.store(false, Ordering::SeqCst);
            }
        }
        let _release = Release(self);
        self.pause.store(true, Ordering::SeqCst);
        let deadline = Instant::now() + Duration::from_millis(800);
        while !self.held.load(Ordering::SeqCst) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        if !self.held.load(Ordering::SeqCst) {
            return Err("selector-6 skipped: stream did not stop".into());
        }
        // STREAMOFF has returned. Wait out the unlink burst before the
        // control transfer, then stay stopped after the write so STREAMON
        // does not race the cancels.
        std::thread::sleep(Duration::from_millis(500));
        let result = body();
        std::thread::sleep(Duration::from_millis(700));
        result
    }
}

fn spawn_ctrl(ev: Sender<Ev>, hold: Arc<StreamHold>) -> Sender<UiMsg> {
    let (tx, rx) = mpsc::channel::<UiMsg>();
    std::thread::spawn(move || {
        let xu = match V4lXu::open_camera() {
            Ok(x) => x,
            Err(e) => {
                let _ = ev.send(Ev::Log(format!("XU open failed: {e}")));
                return;
            }
        };
        let mut d = ObsbotUsb::new(xu);
        match d.boot_status() {
            Ok(b) => {
                let _ = ev.send(Ev::Boot(b));
                let _ = ev.send(Ev::Log(format!("boot_status ack: {b}")));
            }
            Err(e) => {
                let _ = ev.send(Ev::Log(format!("boot_status: {e}")));
            }
        }
        match d.serial() {
            Ok(s) => {
                let _ = ev.send(Ev::Serial(s));
            }
            Err(e) => {
                let _ = ev.send(Ev::Log(format!("startup serial FAILED: {e}")));
            }
        }
        match d.run_status(cmd::RUN_AWAKE) {
            Ok(()) => {
                let _ = ev.send(Ev::Log("wake: sent".into()));
            }
            Err(e) => {
                let _ = ev.send(Ev::Log(format!("wake: {e}")));
            }
        }
        // The camera's own 2-minute timer parked the gimbal and swapped in
        // the sleep picture. Leave it off until the switch is turned on.
        match d.auto_sleep_seconds(0) {
            Ok(()) => {
                let _ = ev.send(Ev::Log("auto-sleep: off".into()));
            }
            Err(e) => {
                let _ = ev.send(Ev::Log(format!("auto-sleep: {e}")));
            }
        }
        match d.preset_names(5) {
            Ok(p) => {
                let _ = ev.send(Ev::Presets(p));
            }
            Err(e) => {
                let _ = ev.send(Ev::Log(format!("startup presets FAILED: {e}")));
            }
        }
        let mut last_poll = Instant::now();
        let mut last_status = Instant::now();
        let mut status_misses = 0u8;
        let mut poll_ok = false;
        let mut poll_err = false;
        loop {
            match rx.recv_timeout(Duration::from_millis(300)) {
                Ok(m) => {
                    let (kind, r): (&str, Result<(), Box<dyn Error>>) = match m {
                        UiMsg::Speed(p, t) => (
                            "speed",
                            d.gimbal_speed(p, t, 0.0).map_err(|e| e.into()),
                        ),
                        UiMsg::Stop => ("stop", d.gimbal_stop().map_err(|e| e.into())),
                        UiMsg::Rst => (
                            "reset",
                            d.gimbal_rst(TINY2_LITE_RESET_MODE).map_err(|e| e.into()),
                        ),
                        UiMsg::Track(on) => {
                            ("ai-track", d.tracking_enable(on).map_err(|e| e.into()))
                        }
                        UiMsg::TrackMode(m2) => {
                            ("track-mode", d.tracking_mode(m2).map_err(|e| e.into()))
                        }
                        UiMsg::TrackType(t) => {
                            ("track-type", d.tracking_type(t).map_err(|e| e.into()))
                        }
                        UiMsg::TrackClear(t) => (
                            "track-clear",
                            d.command(&cmd::tracking_clear(t)).map_err(|e| e.into()),
                        ),
                        UiMsg::AiWork(work, target) => (
                            "ai-work",
                            hold.around(|| {
                                d.ai_field(0x0216, &[work, target]).map_err(|e| e.into())
                            }),
                        ),
                        UiMsg::Goto(id) => {
                            ("preset-goto", d.preset_goto(id).map_err(|e| e.into()))
                        }
                        UiMsg::Tally(on) => (
                            "tally",
                            d.command(&cmd::tally_light(on)).map_err(|e| e.into()),
                        ),
                        UiMsg::Bright(v) => (
                            "tally-bright",
                            d.command(&cmd::tally_brightness(v)).map_err(|e| e.into()),
                        ),
                        UiMsg::Run(status) => (
                            "run",
                            d.run_status(status).map_err(|e| e.into()),
                        ),
                        UiMsg::AutoSleep(seconds) => (
                            "auto-sleep",
                            d.auto_sleep_seconds(seconds).map_err(|e| e.into()),
                        ),
                        UiMsg::FaceFocus(v) => (
                            "face-focus",
                            d.face_focus(v).map_err(|e| e.into()),
                        ),
                        UiMsg::AfMode(v) => {
                            ("af-mode", d.set_af_mode(v).map_err(|e| e.into()))
                        }
                        UiMsg::ExpMode(v) => {
                            ("exp-mode", d.set_exp_mode(v).map_err(|e| e.into()))
                        }
                        UiMsg::Hdr(v) => ("hdr", d.hdr(v, true).map_err(|e| e.into())),
                        UiMsg::Mirror(v) => {
                            ("mirror", d.set_mirror_flip(v).map_err(|e| e.into()))
                        }
                        UiMsg::FaceAe(v) => (
                            "face-ae",
                            hold.around(|| d.ai_field(0x0103, &[v as u8]).map_err(|e| e.into())),
                        ),
                        UiMsg::HandMode(v) => (
                            "hand-mode",
                            d.command(&cmd::hand_track_mode(v)).map_err(|e| e.into()),
                        ),
                        UiMsg::HandGimbal(v) => (
                            "hand-gimbal",
                            d.command(&cmd::hand_track_gimbal(v)).map_err(|e| e.into()),
                        ),
                        UiMsg::YawReverse(v) => (
                            "yaw-reverse",
                            d.command(&cmd::yaw_reverse(v)).map_err(|e| e.into()),
                        ),
                        UiMsg::TrackSpeed(v) => (
                            "track-speed",
                            d.command(&cmd::track_speed(v)).map_err(|e| e.into()),
                        ),
                        UiMsg::TrackOffset { horizontal, value } => {
                            let req = if horizontal {
                                cmd::horizontal_offset(value)
                            } else {
                                cmd::vertical_offset(value)
                            };
                            let kind = if horizontal { "h-offset" } else { "v-offset" };
                            (kind, d.command(&req).map_err(|e| e.into()))
                        }
                        UiMsg::Volume(v) => (
                            "volume",
                            d.command(&cmd::audio_volume(v)).map_err(|e| e.into()),
                        ),
                        UiMsg::AudioField { field, value } => (
                            "audio",
                            hold.around(|| d.ai_field(field, &[value]).map_err(|e| e.into())),
                        ),
                        UiMsg::BgField { field, value } => (
                            "sleep-bg",
                            hold.around(|| d.ai_field(field, &[value]).map_err(|e| e.into())),
                        ),
                        UiMsg::PresetAdd { id, yaw, pitch, zoom } => (
                            "preset-add",
                            d.command(&cmd::preset_add_lite(id, yaw, pitch, zoom))
                                .map_err(|e| e.into()),
                        ),
                        UiMsg::PresetDel(id) => (
                            "preset-del",
                            d.command(&cmd::preset_delete(id)).map_err(|e| e.into()),
                        ),
                        UiMsg::PresetRename { id, name } => (
                            "preset-rename",
                            d.command(&cmd::preset_rename(id, &name)).map_err(|e| e.into()),
                        ),
                    };
                    match r {
                        Ok(()) => {
                            if matches!(kind, "preset-add" | "preset-del" | "preset-rename") {
                                if let Ok(names) = d.preset_names(3) {
                                    let _ = ev.send(Ev::Presets(names));
                                }
                            }
                            eprintln!("ctrl: {kind} ok - camera acked");
                            let _ = ev.send(Ev::Log(format!(
                                "ctrl: {kind} ok - camera acked"
                            )));
                        }
                        Err(e) => {
                            eprintln!("ctrl: {kind} FAILED: {e}");
                            let _ = ev.send(Ev::Log(format!("ctrl: {kind} FAILED: {e}")));
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            if last_poll.elapsed() > Duration::from_millis(500) {
                last_poll = Instant::now();
                match d.attitude() {
                    Ok(a) => {
                        let _ = ev.send(Ev::Attitude(a));
                        if !poll_ok {
                            poll_ok = true;
                            let _ = ev.send(Ev::Log("ctrl: attitude poll ok".into()));
                        }
                    }
                    Err(e) => {
                        if !poll_err {
                            poll_err = true;
                            let _ = ev
                                .send(Ev::Log(format!("ctrl: attitude poll FAILED: {e}")));
                        }
                    }
                }
            }
            if status_misses < 2 && last_status.elapsed() > Duration::from_secs(2) {
                last_status = Instant::now();
                match d.request(&cmd::get_ai_status()) {
                    Ok(frame) => {
                        status_misses = 0;
                        eprintln!("ctrl: {}", summarize_ai_status(&frame.payload));
                        let _ = ev.send(Ev::Status(summarize_ai_status(&frame.payload)));
                    }
                    Err(e) => {
                        status_misses = status_misses.saturating_add(1);
                        if status_misses == 1 {
                            eprintln!("ctrl: ai status GET 13 30 silent: {e}");
                            let _ = ev.send(Ev::Log(format!(
                                "ctrl: ai status GET 13 30 silent: {e}"
                            )));
                        }
                    }
                }
            }
        }
    });
    tx
}

/// Best-effort read of the 0x30-byte AiStatus reply. The product-3 field
/// map is from the decompile; the async 0x89 nibble tables are not used.
fn summarize_ai_status(payload: &[u8]) -> String {
    if payload.len() < 12 {
        return format!("ai status {} B", payload.len());
    }
    let flags: String = payload[..5.min(payload.len())]
        .iter()
        .map(|b| if *b == 0 { '0' } else { '1' })
        .collect();
    let zoom_f = f32::from_le_bytes(payload[8..12].try_into().unwrap_or([0; 4]));
    let zoom = if (0.0..=16.0).contains(&zoom_f) {
        zoom_f
    } else {
        u16::from_le_bytes([payload[8], payload[9]]) as f32 * 0.01
    };
    let mode = if payload.len() >= 0x24 {
        u32::from_le_bytes(payload[0x20..0x24].try_into().unwrap_or([0; 4]))
    } else {
        0
    };
    format!("status {flags} zoom {zoom:.2} mode {mode:#x}")
}

fn spawn_motion(ev: Sender<Ev>, cap_rx: Receiver<CapCmd>) {
    std::thread::spawn(move || {
        let ctrl = match CtrlNode::open() {
            Ok(c) => c,
            Err(e) => {
                let _ = ev.send(Ev::Log(format!("controls: {e}")));
                return;
            }
        };
        let (pan_range, tilt_range) =
            match (ctrl.query_ctrl(CID_PAN_ABSOLUTE), ctrl.query_ctrl(CID_TILT_ABSOLUTE)) {
                (Ok(p), Ok(t)) => (p, t),
                _ => {
                    let _ = ev.send(Ev::Log("gimbal: pan/tilt range failed".into()));
                    return;
                }
            };
        let (mut pan, mut tilt) =
            match (ctrl.get_ctrl(CID_PAN_ABSOLUTE), ctrl.get_ctrl(CID_TILT_ABSOLUTE)) {
                (Ok(p), Ok(t)) => (p, t),
                _ => {
                    let _ = ev.send(Ev::Log("gimbal: pan/tilt read failed".into()));
                    return;
                }
            };
        let _ = ev.send(Ev::Ptz(pan_range, pan, tilt_range, tilt));
        let _ = ev.send(Ev::Log(format!(
            "gimbal: pan {pan} ({}..{}) tilt {tilt} ({}..{})",
            pan_range.minimum, pan_range.maximum, tilt_range.minimum, tilt_range.maximum
        )));
        let mut zoom_range = None;
        match ctrl.query_ctrl(CID_ZOOM_ABSOLUTE) {
            Ok(range) => match ctrl.get_ctrl(CID_ZOOM_ABSOLUTE) {
                Ok(v) => {
                    zoom_range = Some(range);
                    let _ = ev.send(Ev::Zoom(range, v));
                    let _ = ev.send(Ev::Log(format!(
                        "zoom: {v} (range {}..{} step {})",
                        range.minimum, range.maximum, range.step
                    )));
                }
                Err(e) => {
                    let _ = ev.send(Ev::Log(format!("zoom read: {e}")));
                }
            },
            Err(e) => {
                let _ = ev.send(Ev::Log(format!("zoom range: {e}")));
            }
        }
        let mut image = Vec::new();
        for &(id, name) in IMAGE_CTRLS {
            let (Ok(range), Ok(value)) = (ctrl.query_ctrl(id), ctrl.get_ctrl(id)) else {
                continue;
            };
            image.push(ImageCtrl {
                id,
                name,
                range,
                value,
            });
        }
        let _ = ev.send(Ev::Images(image));

        let mut pan_frac = 0.0f32;
        let mut tilt_frac = 0.0f32;
        let mut pan_dps = 0.0f32;
        let mut tilt_dps = 0.0f32;
        let mut tick = std::time::Instant::now();
        let mut zoom_note = String::new();
        loop {
            let mut zoom = None;
            let mut home = false;
            // A tap is a press and a release inside one 8 ms batch. Remember
            // the press so the trailing zero does not erase it before we step.
            let mut pressed = false;
            let mut release_after = false;
            while let Ok(cmd) = cap_rx.try_recv() {
                match cmd {
                    CapCmd::Zoom(v) => zoom = Some(v),
                    CapCmd::Stick(p, t) => {
                        if p != 0.0 || t != 0.0 {
                            pan_dps = p;
                            tilt_dps = t;
                            pressed = true;
                        } else if pressed {
                            release_after = true;
                        } else {
                            pan_dps = 0.0;
                            tilt_dps = 0.0;
                        }
                    }
                    CapCmd::Home => home = true,
                    CapCmd::SetCtrl(id, value) => {
                        if let Err(e) = ctrl.set_ctrl(id, value) {
                            let _ = ev.send(Ev::Log(format!("ctrl {id:#x}: {e}")));
                        }
                    }
                }
            }
            if let Some(v) = zoom {
                let v = match zoom_range {
                    Some(r) => v.clamp(r.minimum, r.maximum),
                    None => v,
                };
                let note = match ctrl.set_ctrl(CID_ZOOM_ABSOLUTE, v) {
                    Ok(()) => {
                        if let (Some(r), Ok(back)) =
                            (zoom_range, ctrl.get_ctrl(CID_ZOOM_ABSOLUTE))
                        {
                            let _ = ev.send(Ev::Zoom(r, back));
                        }
                        format!("zoom {v} ok (V4L2 acked)")
                    }
                    Err(e) => format!("zoom: {e}"),
                };
                if note != zoom_note {
                    zoom_note = note.clone();
                    let _ = ev.send(Ev::Log(note));
                }
            }
            if home {
                pan = 0.clamp(pan_range.minimum, pan_range.maximum);
                tilt = 0.clamp(tilt_range.minimum, tilt_range.maximum);
                pan_frac = 0.0;
                tilt_frac = 0.0;
                pan_dps = 0.0;
                tilt_dps = 0.0;
                if let Err(e) = ctrl.set_ctrl(CID_PAN_ABSOLUTE, pan) {
                    let _ = ev.send(Ev::Log(format!("pan: {e}")));
                }
                if let Err(e) = ctrl.set_ctrl(CID_TILT_ABSOLUTE, tilt) {
                    let _ = ev.send(Ev::Log(format!("tilt: {e}")));
                }
                let _ = ev.send(Ev::Ptz(pan_range, pan, tilt_range, tilt));
            }

            let dt = tick.elapsed().as_secs_f32().min(0.02);
            tick = std::time::Instant::now();
            let before = (pan, tilt);
            let (p, pf) = step_axis(
                pan,
                pan_frac,
                pan_dps,
                dt,
                pan_range.step,
                pan_range.minimum,
                pan_range.maximum,
            );
            let (t, tf) = step_axis(
                tilt,
                tilt_frac,
                tilt_dps,
                dt,
                tilt_range.step,
                tilt_range.minimum,
                tilt_range.maximum,
            );
            pan = p;
            tilt = t;
            pan_frac = pf;
            tilt_frac = tf;
            if release_after && (pan, tilt) == before {
                if pan_dps != 0.0 {
                    let step = pan_range.step * pan_dps.signum() as i32;
                    pan = (pan + step).clamp(pan_range.minimum, pan_range.maximum);
                }
                if tilt_dps != 0.0 {
                    let step = tilt_range.step * tilt_dps.signum() as i32;
                    tilt = (tilt + step).clamp(tilt_range.minimum, tilt_range.maximum);
                }
            }
            if release_after || pan_dps == 0.0 {
                pan_frac = 0.0;
            }
            if release_after || tilt_dps == 0.0 {
                tilt_frac = 0.0;
            }
            if release_after {
                pan_dps = 0.0;
                tilt_dps = 0.0;
            }
            if (pan, tilt) != before {
                if let Err(e) = ctrl.set_ctrl(CID_PAN_ABSOLUTE, pan) {
                    let _ = ev.send(Ev::Log(format!("pan: {e}")));
                }
                if let Err(e) = ctrl.set_ctrl(CID_TILT_ABSOLUTE, tilt) {
                    let _ = ev.send(Ev::Log(format!("tilt: {e}")));
                }
                let _ = ev.send(Ev::Ptz(pan_range, pan, tilt_range, tilt));
            }
            std::thread::sleep(std::time::Duration::from_millis(8));
        }
    });
}

fn spawn_capture(ev: Sender<Ev>, raw_tx: Sender<Vec<u8>>, hold: Arc<StreamHold>) {
    std::thread::spawn(move || {
        // 30 fps is the descriptor default. 60 fps plus a selector-6
        // write, and a second STREAMOFF of a 60 fps stream, both closed
        // the usbip socket.
        let mut cap = match Capture::start_mjpeg_fps(1280, 720, 30) {
            Ok(c) => c,
            Err(e) => {
                let _ = ev.send(Ev::Log(format!("capture failed: {e}")));
                return;
            }
        };
        let _ = ev.send(Ev::Log(format!(
            "capture: {}x{} mjpeg @ {:.2} fps (interval {}/{})",
            cap.w, cap.h, cap.fps, cap.interval_num, cap.interval_den
        )));
        let _ = ev.send(Ev::CamFps(cap.fps));
        loop {
            if hold.pause.load(Ordering::SeqCst) {
                if let Err(e) = cap.suspend() {
                    let _ = ev.send(Ev::Log(format!("capture suspend: {e}")));
                    eprintln!("capture: suspend FAILED: {e}");
                    // `held` stays false, so the writer skips the selector-6
                    // transfer instead of sending it on a live stream.
                    while hold.pause.load(Ordering::SeqCst) {
                        std::thread::sleep(Duration::from_millis(20));
                    }
                    continue;
                }
                eprintln!("capture: stream stopped");
                hold.held.store(true, Ordering::SeqCst);
                while hold.pause.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(5));
                }
                hold.held.store(false, Ordering::SeqCst);
                if let Err(e) = cap.resume() {
                    let _ = ev.send(Ev::Log(format!("capture resume: {e}")));
                    eprintln!("capture: resume FAILED: {e}");
                    break;
                }
                eprintln!("capture: stream resumed at {:.2} fps", cap.fps);
                continue;
            }
            match cap.next_frame() {
                Ok((data, _ts)) => {
                    if raw_tx.send(data).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    let _ = ev.send(Ev::Log(format!("DQBUF: {e}")));
                    break;
                }
            }
        }
    });
}

// ---- the panel --------------------------------------------------------------

struct Panel {
    ev_rx: Mutex<Receiver<Ev>>,
    ui_tx: Sender<UiMsg>,
    cap_tx: Sender<CapCmd>,
    tex: Option<egui::TextureHandle>,
    latest: Option<std::sync::Arc<Decoded>>,
    newest: Option<std::sync::Arc<Decoded>>,
    frames: u64,
    fps: f32,
    fps_win: Instant,
    fps_ct: u32,
    cam_fps: Option<f32>,
    playback_epoch: Instant,
    serial: String,
    boot: Option<bool>,
    attitude: Option<[f32; 3]>,
    presets: Vec<String>,
    log: Vec<String>,
    // control mirror state — combos stay unset until the user picks,
    // so opening the panel does not write a guessed mode to the camera.
    tracking: bool,
    tmode: Option<u8>,
    ttype: Option<u16>,
    tally: bool,
    bright: u32,
    zoom: i32,
    zoom_range: Option<CtrlRange>,
    // device-side controls whose integer meanings the on-device sweep
    // settles; the panel mirrors the last value it sent (GETs for these
    // pairs are silent on Tiny 2 Lite firmware).
    face_focus: bool,
    af_mode: u32,
    exp_mode: u32,
    hdr: bool,
    /// cameraSetMirrorFlipR last value sent (device-side; distinct from the
    /// preview-only `mirror` bool).
    mirror_dev: u32,
    face_ae: bool,
    hand_mode: Option<u8>,
    hand_gimbal: bool,
    /// Center's Hand popover defaults to the right-hand pill. The `49 30`
    /// byte for Left versus Right is not in the binary, so this is only
    /// which pill is lit.
    hand_right: bool,
    /// The Hand button opens this menu. Tracking stays as it is when the
    /// menu closes.
    hand_menu: bool,
    track_level: u8,
    h_off: f32,
    v_off: f32,
    volume: u8,
    agc: bool,
    noise: u8,
    mic_off: bool,
    mic_dist: u8,
    mic_sleep: bool,
    bg_on: bool,
    bg_color: u8,
    preset_slot: u32,
    preset_edit: String,
    ai_line: String,
    pan: i32,
    tilt: i32,
    pan_range: Option<CtrlRange>,
    tilt_range: Option<CtrlRange>,
    speed: f32,
    stick_live: bool,
    saved: u32,
    save_dir: String,
    styled: bool,
    tab: u8,
    preview_open: bool,
    /// Camera run-status. The sleep picture is the device's own output;
    /// this flag is only the button, which sends wake or sleep.
    asleep: bool,
    auto_sleep: bool,
    /// Sleep parks the gimbal. Recenter once when the window opens.
    opened: bool,
    /// Quarter turns clockwise.
    rotation: i32,
    mirror: bool,
    gimbal_reverse: bool,
    recording: bool,
    rec_tick: Instant,
    sleep_bg: u8,
    pad_locked: bool,
    image: Vec<ImageCtrl>,
}

impl Panel {
    fn new(ev_rx: Receiver<Ev>, ui_tx: Sender<UiMsg>, cap_tx: Sender<CapCmd>) -> Self {
        Self {
            ev_rx: Mutex::new(ev_rx),
            ui_tx,
            cap_tx,
            tex: None,
            latest: None,
            newest: None,
            frames: 0,
            fps: 0.0,
            fps_win: Instant::now(),
            fps_ct: 0,
            cam_fps: None,
            playback_epoch: Instant::now(),
            serial: String::new(),
            boot: None,
            attitude: None,
            presets: Vec::new(),
            log: Vec::new(),
            tracking: false,
            tmode: None,
            ttype: None,
            tally: false,
            bright: TALLY_BRIGHT_MAX / 2,
            zoom: 0,
            zoom_range: None,
            face_focus: false,
            af_mode: 0,
            exp_mode: 0,
            hdr: false,
            mirror_dev: 0,
            face_ae: false,
            hand_mode: None,
            hand_gimbal: false,
            hand_right: true,
            hand_menu: false,
            track_level: 255,
            h_off: 0.0,
            v_off: 0.0,
            volume: 50,
            agc: false,
            noise: 255,
            mic_off: false,
            mic_dist: 255,
            mic_sleep: false,
            bg_on: false,
            bg_color: 255,
            preset_slot: 0,
            preset_edit: String::new(),
            ai_line: "status —".into(),
            pan: 0,
            tilt: 0,
            pan_range: None,
            tilt_range: None,
            speed: 0.5,
            stick_live: false,
            saved: 0,
            save_dir: "/tmp/obsbot_panel".into(),
            styled: false,
            tab: 0,
            preview_open: true,
            asleep: false,
            auto_sleep: false,
            opened: false,
            rotation: 0,
            mirror: false,
            gimbal_reverse: false,
            recording: false,
            rec_tick: Instant::now(),
            sleep_bg: 255,
            pad_locked: false,
            image: Vec::new(),
        }
    }

    fn drain_events(&mut self) {
        let rx = self.ev_rx.get_mut().unwrap();
        while let Ok(e) = rx.try_recv() {
            match e {
                Ev::Frame(d) => {
                    self.latest = Some(d.clone());
                    self.newest = Some(d);
                }
                Ev::CamFps(hz) => self.cam_fps = Some(hz),
                Ev::Zoom(range, value) => {
                    self.zoom = value.clamp(range.minimum, range.maximum);
                    self.zoom_range = Some(range);
                }
                Ev::Ptz(pan_range, pan, tilt_range, tilt) => {
                    self.pan = pan.clamp(pan_range.minimum, pan_range.maximum);
                    self.tilt = tilt.clamp(tilt_range.minimum, tilt_range.maximum);
                    self.pan_range = Some(pan_range);
                    self.tilt_range = Some(tilt_range);
                }
                Ev::Images(list) => self.image = list,
                Ev::Log(s) => {
                    self.log.push(s);
                    if self.log.len() > 40 {
                        self.log.remove(0);
                    }
                }
                Ev::Serial(s) => self.serial = s,
                Ev::Boot(b) => self.boot = Some(b),
                Ev::Attitude(a) => self.attitude = Some(a),
                Ev::Presets(p) => self.presets = p,
                Ev::Status(s) => self.ai_line = s,
            }
        }
    }

    /// Upload the newest decoded frame. The fps counter is this rate,
    /// not how many JPEGs were sitting in the queue.
    fn update_texture(&mut self, ctx: &egui::Context) {
        let Some(d) = self.newest.take() else { return };
        match &mut self.tex {
            Some(t) => t.set(
                egui::ImageData::Color(d.img.clone()),
                egui::TextureOptions::LINEAR,
            ),
            None => {
                self.tex = Some(ctx.load_texture(
                    "obsbot_frame",
                    egui::ImageData::Color(d.img.clone()),
                    egui::TextureOptions::LINEAR,
                ));
            }
        }
        self.frames += 1;
        self.fps_ct += 1;
        let dt = self.fps_win.elapsed().as_secs_f32();
        if dt >= 1.0 {
            self.fps = self.fps_ct as f32 / dt;
            self.fps_ct = 0;
            self.fps_win = Instant::now();
        }
    }

    fn apply_theme(&mut self, ctx: &egui::Context) {
        if self.styled {
            return;
        }
        self.styled = true;
        let mut visuals = egui::Visuals::dark();
        visuals.window_fill = BG;
        visuals.panel_fill = PANEL;
        visuals.extreme_bg_color = BG;
        visuals.faint_bg_color = CARD;
        visuals.override_text_color = Some(TEXT);
        visuals.selection.bg_fill = RED;
        visuals.selection.stroke = egui::Stroke::new(1.0, RED);
        visuals.hyperlink_color = RED;
        visuals.window_corner_radius = egui::CornerRadius::same(10);
        visuals.menu_corner_radius = egui::CornerRadius::same(8);
        visuals.collapsing_header_frame = false;
        visuals.indent_has_left_vline = false;
        visuals.slider_trailing_fill = false;
        visuals.widgets.noninteractive.fg_stroke.color = TEXT;
        visuals.widgets.noninteractive.bg_fill = PANEL;
        visuals.widgets.inactive.fg_stroke.color = TEXT;
        visuals.widgets.inactive.bg_fill = CARD;
        visuals.widgets.inactive.weak_bg_fill = CARD;
        visuals.widgets.hovered.fg_stroke.color = TEXT;
        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(58, 58, 64);
        visuals.widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(58, 58, 64);
        visuals.widgets.active.fg_stroke.color = egui::Color32::WHITE;
        visuals.widgets.active.bg_fill = RED;
        visuals.widgets.active.weak_bg_fill = RED;
        visuals.widgets.open.fg_stroke.color = TEXT;
        visuals.widgets.open.bg_fill = CARD;
        ctx.set_visuals(visuals);
        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        ctx.set_style(style);
    }

    fn send_stick(&mut self, x: f32, y: f32) {
        if self.pad_locked {
            return;
        }
        let (pan, tilt) = stick_axes(x, y, self.speed, self.gimbal_reverse);
        let _ = self.cap_tx.send(CapCmd::Stick(
            pan * GIMBAL_MAX_DEG_PER_SEC,
            tilt * GIMBAL_MAX_DEG_PER_SEC,
        ));
        self.stick_live = true;
    }

    fn release_stick(&mut self) {
        if self.stick_live {
            let _ = self.cap_tx.send(CapCmd::Stick(0.0, 0.0));
            self.stick_live = false;
        }
    }

    /// Circular velocity pad. The knob follows the pointer and springs back
    /// on release, which is the same stick the control thread already steps.
    fn gimbal_pad(&mut self, ui: &mut egui::Ui) {
        let size = 148.0;
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click_and_drag());
        let center = rect.center();
        let radius = size * 0.46;
        let painter = ui.painter();
        painter.circle_filled(center, radius, egui::Color32::from_rgb(36, 36, 40));
        painter.circle_stroke(center, radius, egui::Stroke::new(1.0, egui::Color32::from_rgb(72, 72, 78)));
        let caret = MUTED;
        for dir in [egui::vec2(0.0, -1.0), egui::vec2(1.0, 0.0), egui::vec2(0.0, 1.0), egui::vec2(-1.0, 0.0)] {
            draw_caret(painter, center + dir * (radius - 16.0), dir, caret);
        }
        let reach = radius * 0.62;
        let mut knob = center;
        let held = resp.dragged() || resp.is_pointer_button_down_on();
        if held {
            if let Some(p) = resp.interact_pointer_pos() {
                let mut delta = p - center;
                let mag = delta.length();
                if mag > reach && mag > 0.0 {
                    delta = delta / mag * reach;
                }
                knob = center + delta;
                if mag > 10.0 {
                    self.send_stick(delta.x / reach, -delta.y / reach);
                }
            }
        } else if resp.clicked() {
            let near_center = resp
                .interact_pointer_pos()
                .map(|p| p.distance(center) < 24.0)
                .unwrap_or(false);
            self.release_stick();
            if near_center {
                self.home_gimbal();
            }
        } else {
            self.release_stick();
        }
        painter.circle_filled(knob, 22.0, egui::Color32::from_rgb(232, 232, 236));
        painter.circle_stroke(knob, 22.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(180, 180, 186)));
        resp.on_hover_text("Drag to move. Click to recenter.");
    }

    fn home_gimbal(&mut self) {
        self.stick_live = false;
        self.pan = 0;
        self.tilt = 0;
        let _ = self.cap_tx.send(CapCmd::Stick(0.0, 0.0));
        let _ = self.cap_tx.send(CapCmd::Home);
        let _ = self.ui_tx.send(UiMsg::Rst);
    }

    fn set_image(&mut self, index: usize, value: i32) {
        let Some(ctrl) = self.image.get_mut(index) else { return };
        let value = value.clamp(ctrl.range.minimum, ctrl.range.maximum);
        ctrl.value = value;
        let id = ctrl.id;
        let _ = self.cap_tx.send(CapCmd::SetCtrl(id, value));
    }

    fn track_person(&mut self) {
        if self.hand_mode.is_none() && self.tracking && self.ttype == Some(1) {
            self.tracking = false;
            self.ttype = None;
            let _ = self.ui_tx.send(UiMsg::TrackClear(1));
            let _ = self.ui_tx.send(UiMsg::Track(false));
            let _ = self.ui_tx.send(UiMsg::AiWork(0, 0));
            return;
        }
        // Work mode 2 replaces hand's work mode 3, so the hand bytes go off
        // without a second selector-6 write.
        self.release_hand(false);
        if self.tracking {
            let mode = self.ttype.unwrap_or(0x1E);
            let _ = self.ui_tx.send(UiMsg::TrackClear(mode));
            let _ = self.ui_tx.send(UiMsg::Track(false));
        }
        self.tracking = true;
        self.ttype = Some(1);
        // OBSBOT_Main's product-3 jump table, index 1, calls
        // cameraSetAiModeU(2, 1) before the gimbal will follow. `71 30` alone acks
        // and does not move the head.
        let _ = self.ui_tx.send(UiMsg::AiWork(2, 1));
        let _ = self.ui_tx.send(UiMsg::Track(true));
        let _ = self.ui_tx.send(UiMsg::TrackType(1));
    }

    fn track_group(&mut self) {
        if self.hand_mode.is_none() && self.tracking && self.ttype == Some(0x1E) {
            self.tracking = false;
            self.ttype = None;
            let _ = self.ui_tx.send(UiMsg::TrackClear(0x1E));
            let _ = self.ui_tx.send(UiMsg::Track(false));
            return;
        }
        let from_hand = self.hand_mode.is_some();
        self.release_hand(false);
        if self.tracking {
            let mode = self.ttype.unwrap_or(1);
            let _ = self.ui_tx.send(UiMsg::TrackClear(mode));
            let _ = self.ui_tx.send(UiMsg::Track(false));
        }
        // Group has no proven work-mode byte. Leaving hand must still clear
        // work mode 3 or the head stays in the hand program.
        if from_hand {
            let _ = self.ui_tx.send(UiMsg::AiWork(0, 0));
        }
        self.tracking = true;
        self.ttype = Some(0x1E);
        let _ = self.ui_tx.send(UiMsg::Track(true));
        let _ = self.ui_tx.send(UiMsg::TrackType(0x1E));
    }

    fn set_hand_tracking(&mut self, on: bool) {
        if on == self.hand_mode.is_some() {
            return;
        }
        if !on {
            self.release_hand(true);
            return;
        }
        if self.tracking {
            let mode = self.ttype.unwrap_or(1);
            let _ = self.ui_tx.send(UiMsg::TrackClear(mode));
            let _ = self.ui_tx.send(UiMsg::Track(false));
            self.tracking = false;
            self.ttype = None;
        }
        self.hand_mode = Some(1);
        self.hand_gimbal = true;
        // Proven hand-on path: work mode 3, then `49 30` byte 1, then gimbal follow.
        let _ = self.ui_tx.send(UiMsg::AiWork(3, 0));
        let _ = self.ui_tx.send(UiMsg::HandMode(1));
        let _ = self.ui_tx.send(UiMsg::HandGimbal(true));
    }

    fn set_hand_side(&mut self, right: bool) {
        self.hand_right = right;
    }

    /// `49 30` byte 0 and gimbal-follow off. `clear_work` also sends work
    /// mode 0. Callers that immediately set another work mode pass false so
    /// the stream pauses once.
    fn release_hand(&mut self, clear_work: bool) {
        if self.hand_mode.is_none() && !self.hand_gimbal {
            return;
        }
        let _ = self.ui_tx.send(UiMsg::HandMode(0));
        let _ = self.ui_tx.send(UiMsg::HandGimbal(false));
        self.hand_mode = None;
        self.hand_gimbal = false;
        if clear_work {
            let _ = self.ui_tx.send(UiMsg::AiWork(0, 0));
        }
    }

    fn save_frame(&mut self) {
        let Some(latest) = self.latest.clone() else { return };
        self.saved += 1;
        let path = format!("{}/shot_{}.jpg", self.save_dir, self.saved);
        let _ = std::fs::create_dir_all(&self.save_dir);
        if std::fs::write(&path, latest.raw.as_slice()).is_ok() {
            self.log.push(format!("saved {path} ({} B)", latest.raw.len()));
        }
    }

    fn open_folder(&mut self) {
        let _ = std::fs::create_dir_all(&self.save_dir);
        let _ = std::process::Command::new("xdg-open").arg(&self.save_dir).spawn();
        self.log.push(format!("folder {}", self.save_dir));
    }

    fn export_log(&mut self) {
        let _ = std::fs::create_dir_all(&self.save_dir);
        let path = format!("{}/panel.log", self.save_dir);
        if std::fs::write(&path, self.log.join("\n")).is_ok() {
            self.log.push(format!("log {path}"));
        }
    }

    fn speed_and_zoom(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_max_width((ui.available_width() - 156.0).max(90.0));
                ui.label(egui::RichText::new("Gimbal Speed").size(13.0));
                let span = ui.available_width();
                let (row, _) = ui.allocate_exact_size(egui::vec2(span, 16.0), egui::Sense::hover());
                let p = ui.painter();
                for (t, label) in [(0.0, "Slow"), (0.5, "Med."), (1.0, "Fast")] {
                    p.text(
                        egui::pos2(row.left() + span * t, row.center().y),
                        if t == 0.0 {
                            egui::Align2::LEFT_CENTER
                        } else if t == 1.0 {
                            egui::Align2::RIGHT_CENTER
                        } else {
                            egui::Align2::CENTER_CENTER
                        },
                        label,
                        egui::FontId::proportional(11.0),
                        MUTED,
                    );
                }
                ui.spacing_mut().slider_width = ui.available_width();
                ui.add(
                    egui::Slider::new(&mut self.speed, 0.1..=1.0)
                        .smart_aim(false)
                        .show_value(false),
                );
                ui.add_space(6.0);
                ui.label(egui::RichText::new("Manual Zoom").size(13.0));
                self.zoom_row(ui);
            });
            self.gimbal_pad(ui);
        });
        if self.pan_range.is_some() {
            ui.label(
                egui::RichText::new(format!(
                    "pan {:.0}°    tilt {:.0}°",
                    self.pan as f32 / ARCSEC_PER_DEG,
                    self.tilt as f32 / ARCSEC_PER_DEG
                ))
                .size(11.0)
                .color(MUTED),
            );
        }
    }

    fn zoom_row(&mut self, ui: &mut egui::Ui) {
        let Some(range) = self.zoom_range else {
            ui.label(egui::RichText::new("waiting for zoom…").color(MUTED).size(12.0));
            return;
        };
        let times = zoom_times(self.zoom, range.minimum, range.maximum);
        let step = range.step.max(1) as f64;
        ui.horizontal(|ui| {
            ui.spacing_mut().slider_width = (ui.available_width() - 72.0).max(48.0);
            if ui
                .add(
                    egui::Slider::new(&mut self.zoom, range.minimum..=range.maximum)
                        .step_by(step)
                        .smart_aim(false)
                        .show_value(false),
                )
                .changed()
            {
                let _ = self.cap_tx.send(CapCmd::Zoom(self.zoom));
            }
            value_chip(ui, &format!("{times:.2} x"));
        });
    }

    fn image_tab(&mut self, ui: &mut egui::Ui) {
        section(ui, "Sensor", |ui| {
            if switch_row(ui, "HDR / WDR (pair 14 10)", self.hdr) {
                self.hdr = !self.hdr;
                let _ = self.ui_tx.send(UiMsg::Hdr(self.hdr));
            }
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Orientation (2E 10)").size(13.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button("\u{25B8}")
                        .on_hover_text("cycle cameraSetMirrorFlipR 0..3; mirror/flip/both mapping is settled by the preview")
                        .clicked()
                    {
                        self.mirror_dev = (self.mirror_dev + 1) % 4;
                        let _ = self.ui_tx.send(UiMsg::Mirror(self.mirror_dev));
                    }
                    value_chip(ui, &self.mirror_dev.to_string());
                });
            });
        });
        section(ui, "Preset", |ui| {
            ui.horizontal(|ui| {
                for i in 0..3u32 {
                    let fallback = if i == 0 { "Preset1".to_string() } else { format!("P{}", i + 1) };
                    let name = self
                        .presets
                        .get(i as usize)
                        .filter(|s| !s.is_empty())
                        .cloned()
                        .unwrap_or(fallback);
                    if pill(ui, &name, self.preset_slot == i).clicked() {
                        self.preset_slot = i;
                        let _ = self.ui_tx.send(UiMsg::Goto(i));
                    }
                }
            });
            ui.horizontal(|ui| {
                if ui.button("Save pose").clicked() {
                    let yaw = self.pan as f32 / 3600.0;
                    let pitch = self.tilt as f32 / 3600.0;
                    let zoom = self
                        .zoom_range
                        .map(|r| zoom_times(self.zoom, r.minimum, r.maximum))
                        .unwrap_or(1.0);
                    let _ = self.ui_tx.send(UiMsg::PresetAdd {
                        id: self.preset_slot,
                        yaw,
                        pitch,
                        zoom,
                    });
                }
                if ui.button("Delete").clicked() {
                    let _ = self.ui_tx.send(UiMsg::PresetDel(self.preset_slot));
                }
            });
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.preset_edit).desired_width(120.0));
                if ui.button("Rename").clicked() {
                    let name = self.preset_edit.trim().to_string();
                    if !name.is_empty() {
                        let _ = self.ui_tx.send(UiMsg::PresetRename {
                            id: self.preset_slot,
                            name,
                        });
                    }
                }
            });
            ui.label(
                egui::RichText::new("Goto stays on 04 3B. Save is 37 30, delete 38 30, rename 3D 30. Pose axis order is unverified.")
                    .size(11.0)
                    .color(MUTED),
            );
        });
        section(ui, "Tracking", |ui| {
            ui.horizontal(|ui| {
                ui.label("Track speed");
                for (level, name) in [(0u8, "Slow"), (1, "Med"), (2, "Fast")] {
                    if pill(ui, name, self.track_level == level).clicked() {
                        self.track_level = level;
                        let _ = self.ui_tx.send(UiMsg::TrackSpeed(level));
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label("H");
                if ui.add(egui::Slider::new(&mut self.h_off, -1.0..=1.0)).changed() {
                    let _ = self.ui_tx.send(UiMsg::TrackOffset {
                        horizontal: true,
                        value: self.h_off,
                    });
                }
            });
            ui.horizontal(|ui| {
                ui.label("V");
                if ui.add(egui::Slider::new(&mut self.v_off, -1.0..=1.0)).changed() {
                    let _ = self.ui_tx.send(UiMsg::TrackOffset {
                        horizontal: false,
                        value: self.v_off,
                    });
                }
            });
        });
        section(ui, "View and Gimbal", |ui| {
            self.speed_and_zoom(ui);
        });
        section(ui, "Beauty", |ui| {
            ui.add_enabled_ui(false, |ui| {
                switch_row(ui, "Beauty", false);
                ui.horizontal(|ui| {
                    let _ = pill(ui, "Mode", false);
                    let _ = pill(ui, "Picture", false);
                    let _ = pill(ui, "Smooth", false);
                });
            })
            .response
            .on_disabled_hover_text("Beauty runs in the host filter (rm.dll). It is not a camera command.");
        });
        section(ui, "Image Adj.", |ui| {
            ui.add_enabled_ui(false, |ui| {
                switch_row(ui, "HDR", false);
            });
            let count = self.image.len();
            for index in 0..count {
                if matches!(
                    self.image[index].id,
                    CID_CONTRAST | CID_SATURATION | CID_SHARPNESS | CID_HUE
                ) {
                    continue;
                }
                self.show_image_ctrl(ui, index);
            }
            if self.image.is_empty() {
                ui.label(egui::RichText::new("waiting for image controls…").color(MUTED).size(12.0));
            }
        });
        section(ui, "Image", |ui| {
            let count = self.image.len();
            let mut any = false;
            for index in 0..count {
                if !matches!(
                    self.image[index].id,
                    CID_CONTRAST | CID_SATURATION | CID_SHARPNESS | CID_HUE
                ) {
                    continue;
                }
                any = true;
                self.show_image_ctrl(ui, index);
            }
            if !any {
                ui.label(egui::RichText::new("waiting for image controls…").color(MUTED).size(12.0));
            }
        });
    }

    fn show_image_ctrl(&mut self, ui: &mut egui::Ui, index: usize) {
        let ctrl = self.image[index].clone();
        match ctrl.id {
            CID_POWER_LINE_FREQUENCY => {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Anti-Flicker").size(13.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        for (name, value) in [("60 Hz", 2), ("50 Hz", 1), ("Off", 0)] {
                            if pill(ui, name, ctrl.value == value).clicked() {
                                self.set_image(index, value);
                            }
                        }
                    });
                });
            }
            CID_EXPOSURE_AUTO => {
                let on = ctrl.value != 1;
                if switch_row(ui, "Auto Exposure", on) {
                    self.set_image(index, if on { 1 } else { 0 });
                }
                if let Some(mode) =
                    face_global(ui, "Auto Exposure Mode", self.exp_mode, &self.ui_tx, UiMsg::ExpMode)
                {
                    self.exp_mode = mode;
                }
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Face AE (sel6 0103)").size(12.0).color(MUTED));
                    if pill(ui, if self.face_ae { "On" } else { "Off" }, self.face_ae).clicked() {
                        self.face_ae = !self.face_ae;
                        let _ = self.ui_tx.send(UiMsg::FaceAe(self.face_ae));
                    }
                });
            }
            CID_FOCUS_AUTO => {
                let on = ctrl.value != 0;
                if switch_row(ui, "Auto Focus", on) {
                    self.set_image(index, if on { 0 } else { 1 });
                }
                if let Some(mode) =
                    face_global(ui, "Auto Focus Mode", self.af_mode, &self.ui_tx, UiMsg::AfMode)
                {
                    self.af_mode = mode;
                }
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Vendor Face Focus (03 10)").size(12.0).color(MUTED));
                    if pill(ui, if self.face_focus { "On" } else { "Off" }, self.face_focus)
                        .clicked()
                    {
                        self.face_focus = !self.face_focus;
                        let _ = self.ui_tx.send(UiMsg::FaceFocus(self.face_focus));
                    }
                });
            }
            CID_AUTO_WHITE_BALANCE => {
                let on = ctrl.value != 0;
                if switch_row(ui, "Auto WB", on) {
                    self.set_image(index, if on { 0 } else { 1 });
                }
            }
            _ => {
                let suffix = if ctrl.id == CID_WHITE_BALANCE_TEMPERATURE { " k" } else { "" };
                ui.label(egui::RichText::new(ctrl.name).size(13.0));
                let mut value = ctrl.value;
                if slider_row(ui, &mut value, ctrl.range, suffix) {
                    self.set_image(index, value);
                }
            }
        }
    }

    fn more_tab(&mut self, ui: &mut egui::Ui) {
        section(ui, "Audio", |ui| {
            let mut volume = self.volume as i32;
            let range = CtrlRange {
                minimum: 0,
                maximum: 100,
                step: 1,
                default_value: 50,
            };
            if slider_row(ui, &mut volume, range, "") {
                self.volume = volume as u8;
                let _ = self.ui_tx.send(UiMsg::Volume(self.volume));
            }
            ui.horizontal(|ui| {
                ui.label("Noise");
                for (level, name) in [(0u8, "Off"), (1, "Weak"), (2, "Med"), (3, "Strong")] {
                    if pill(ui, name, self.noise == level).clicked() {
                        self.noise = level;
                        let _ = self.ui_tx.send(UiMsg::AudioField {
                            field: 0x010A,
                            value: level,
                        });
                    }
                }
            });
            if switch_row(ui, "Auto Gain", self.agc) {
                self.agc = !self.agc;
                let _ = self.ui_tx.send(UiMsg::AudioField {
                    field: 0x0117,
                    value: self.agc as u8,
                });
            }
            if switch_row(ui, "Disable Microphone", self.mic_off) {
                self.mic_off = !self.mic_off;
                let _ = self.ui_tx.send(UiMsg::AudioField {
                    field: 0x011C,
                    value: cmd::mic_enabled_byte(self.mic_off),
                });
            }
            if switch_row(ui, "Mic During Sleep", self.mic_sleep) {
                self.mic_sleep = !self.mic_sleep;
                let _ = self.ui_tx.send(UiMsg::AudioField {
                    field: 0x0113,
                    value: self.mic_sleep as u8,
                });
            }
            ui.horizontal(|ui| {
                ui.label("Distance");
                for (level, name) in [(0u8, "Close"), (1, "Mid"), (2, "Far")] {
                    if pill(ui, name, self.mic_dist == level).clicked() {
                        self.mic_dist = level;
                        let _ = self.ui_tx.send(UiMsg::AudioField {
                            field: 0x011B,
                            value: level,
                        });
                    }
                }
            });
        });
        section(ui, "Device Sleep", |ui| {
            if switch_row(ui, "Auto Sleep", self.auto_sleep) {
                self.auto_sleep = !self.auto_sleep;
                let seconds = if self.auto_sleep { 120 } else { 0 };
                let _ = self.ui_tx.send(UiMsg::AutoSleep(seconds));
            }
            ui.label(
                egui::RichText::new("Off until you turn it on. Two minutes is the timer Center was using. Sleep parks the gimbal down and shows the camera's own background.")
                    .size(11.0)
                    .color(MUTED),
            );
            if switch_row(ui, "Sleep Background", self.bg_on) {
                self.bg_on = !self.bg_on;
                let _ = self.ui_tx.send(UiMsg::BgField {
                    field: 0x0111,
                    value: cmd::bg_disabled_byte(self.bg_on),
                });
            }
            ui.horizontal(|ui| {
                ui.label("Mode");
                for mode in 0..3u8 {
                    if pill(ui, &mode.to_string(), self.sleep_bg == mode).clicked() {
                        self.sleep_bg = mode;
                        let _ = self.ui_tx.send(UiMsg::BgField {
                            field: 0x0105,
                            value: mode,
                        });
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Color");
                for color in 0..3u8 {
                    if pill(ui, &color.to_string(), self.bg_color == color).clicked() {
                        self.bg_color = color;
                        let _ = self.ui_tx.send(UiMsg::BgField {
                            field: 0x0110,
                            value: color,
                        });
                    }
                }
            });
        });
        section(ui, "Indicator Status", |ui| {
            if switch_row(ui, "Status Light", self.tally) {
                self.tally = !self.tally;
                let _ = self.ui_tx.send(UiMsg::Tally(self.tally));
            }
            ui.label(egui::RichText::new("Brightness").size(13.0));
            let mut bright = self.bright as i32;
            let range = CtrlRange {
                minimum: 0,
                maximum: TALLY_BRIGHT_MAX as i32,
                step: 1,
                default_value: (TALLY_BRIGHT_MAX / 2) as i32,
            };
            if slider_row(ui, &mut bright, range, "") {
                self.bright = bright as u32;
                let _ = self.ui_tx.send(UiMsg::Bright(self.bright));
            }
        });
        section(ui, "More Settings", |ui| {
            if switch_row(ui, "Yaw Reverse", self.gimbal_reverse) {
                self.gimbal_reverse = !self.gimbal_reverse;
                let _ = self.ui_tx.send(UiMsg::YawReverse(self.gimbal_reverse));
            }
            ui.horizontal(|ui| {
                ui.add_enabled(false, egui::Button::new("Feedback"));
                if ui.button("Export Log").clicked() {
                    self.export_log();
                }
            });
            ui.horizontal(|ui| {
                ui.add_enabled(false, egui::Button::new("Factory Reset"))
                    .on_disabled_hover_text("Factory reset is not sent by this panel.");
                ui.add_enabled(false, egui::Button::new("Manual Update"));
            });
        });
    }

    fn preview_top(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let _ = pill(ui, "Tiny 2 Lite", false);
            if self.tracking {
                ui.colored_label(RED, "●");
            }
            let sleep_label = if self.asleep { "Wake" } else { "Sleep" };
            if pill(ui, sleep_label, self.asleep).clicked() {
                self.release_stick();
                if self.asleep {
                    let _ = self.ui_tx.send(UiMsg::Run(cmd::RUN_AWAKE));
                    self.asleep = false;
                    self.home_gimbal();
                } else {
                    let _ = self.ui_tx.send(UiMsg::Run(cmd::RUN_SLEEP));
                    self.asleep = true;
                }
            }
            if pill(ui, "Close Preview", !self.preview_open).clicked() {
                self.preview_open = !self.preview_open;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(32, 32, 36))
                    .corner_radius(egui::CornerRadius::same(8))
                    .inner_margin(8)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("How do I use the current video in another app?").size(12.0));
                        ui.label(
                            egui::RichText::new("This window is the preview. A virtual camera is not part of this build.")
                                .size(11.0)
                                .color(MUTED),
                        );
                    });
            });
        });
    }

    fn preview_bottom(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let _ = pill(ui, "16:9", true);
            let fps_label = match self.cam_fps {
                Some(hz) => format!("720p {hz:.0}"),
                None => "720p".to_string(),
            };
            ui.menu_button(fps_label, |ui| {
                ui.add_enabled(false, egui::Button::new("4K (Ultra HD)"));
                ui.add_enabled(false, egui::Button::new("1080P (Full HD)"));
                ui.label("720P (HD)   ✓");
                ui.separator();
                ui.label("30 (Smooth)   ✓");
                ui.add_enabled(false, egui::Button::new("60"));
                ui.separator();
                ui.label("Landscape (16:9)   ✓");
                ui.add_enabled(false, egui::Button::new("Portrait (9:16)"));
            })
            .response
            .on_hover_text("This panel streams 1280×720 at 30 fps. 60 fps on usbip drops the camera when a selector-6 control is sent.");
            if icon_button(ui, "CCW Rotate 90°", false, draw_rotate_ccw).clicked() {
                self.rotation = (self.rotation + 3) % 4;
            }
            if icon_button(ui, "CW Rotate 90°", false, draw_rotate_cw).clicked() {
                self.rotation = (self.rotation + 1) % 4;
            }
            if icon_button(ui, "Mirror Image", self.mirror, draw_mirror).clicked() {
                self.mirror = !self.mirror;
            }
            let help = self.help_text();
            icon_button(ui, &help, false, draw_help);
            self.ai_modes(ui);
            if icon_button(ui, "Lock gimbal pad", self.pad_locked, draw_lock).clicked() {
                self.pad_locked = !self.pad_locked;
                self.release_stick();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icon_button(ui, "Disable microphone (011C, inverted)", self.mic_off, draw_mic)
                    .clicked()
                {
                    self.mic_off = !self.mic_off;
                    let _ = self.ui_tx.send(UiMsg::AudioField {
                        field: 0x011C,
                        value: cmd::mic_enabled_byte(self.mic_off),
                    });
                }
                if icon_button(ui, "Open Folder", false, draw_folder).clicked() {
                    self.open_folder();
                }
                if icon_button(ui, "Snapshot", false, draw_camera).clicked() {
                    self.save_frame();
                }
                if icon_button(ui, "Record JPEG frames", self.recording, draw_record).clicked() {
                    self.recording = !self.recording;
                    self.rec_tick = Instant::now();
                }
            });
        });
    }

    fn ai_modes(&mut self, ui: &mut egui::Ui) {
        let human_on = self.hand_mode.is_none() && self.tracking && self.ttype == Some(1);
        let group_on = self.hand_mode.is_none() && self.tracking && self.ttype == Some(0x1E);
        let hand_on = self.hand_mode.is_some();
        let cluster = ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let (rect, resp) = ui.allocate_exact_size(egui::vec2(58.0, 56.0), egui::Sense::click());
            paint_mode_fill(ui.painter(), rect, human_on, resp.hovered());
            let icon = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), 34.0));
            draw_person(ui.painter(), icon);
            ui.painter().text(
                egui::pos2(rect.center().x, rect.bottom() - 11.0),
                egui::Align2::CENTER_CENTER,
                "Human",
                egui::FontId::proportional(11.0),
                TEXT,
            );
            if resp.on_hover_text("Human Tracking").clicked() {
                self.track_person();
            }

            let hand_resp = ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                if mode_row(ui, 108.0, "Group", group_on, false, draw_group)
                    .on_hover_text("Group")
                    .clicked()
                {
                    self.track_group();
                }
                let hand_resp = mode_row(ui, 108.0, "Hand", hand_on, true, draw_hand)
                    .on_hover_text("Hand Tracking");
                if hand_resp.clicked() {
                    self.hand_menu = !self.hand_menu;
                }
                hand_resp
            });
            hand_resp.inner
        });
        let hand_resp = cluster.inner;
        if self.hand_menu {
            let popup = self.hand_popover(ui.ctx(), hand_resp.rect);
            let dismiss = hand_resp.clicked_elsewhere() && popup.clicked_elsewhere();
            if dismiss || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.hand_menu = false;
            }
        }
    }

    fn hand_popover(&mut self, ctx: &egui::Context, anchor: egui::Rect) -> egui::Response {
        let pos = anchor.left_top() + egui::vec2(0.0, -6.0);
        egui::Area::new(egui::Id::new("hand_tracking_popover"))
            .order(egui::Order::Foreground)
            .movable(false)
            .sense(egui::Sense::click())
            .fixed_pos(pos)
            .pivot(egui::Align2::LEFT_BOTTOM)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(20, 20, 22))
                    .corner_radius(egui::CornerRadius::same(10))
                    .inner_margin(12)
                    .show(ui, |ui| {
                        ui.set_min_width(236.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Tracking").size(13.0).color(TEXT));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if side_pill(ui, "On", self.hand_mode.is_some()).clicked() {
                                    self.set_hand_tracking(true);
                                }
                                if side_pill(ui, "Off", self.hand_mode.is_none()).clicked() {
                                    self.set_hand_tracking(false);
                                }
                            });
                        });
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Hand Tracking").size(13.0).color(TEXT));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if side_pill(ui, "Right", self.hand_right).clicked() {
                                    self.set_hand_side(true);
                                }
                                if side_pill(ui, "Left", !self.hand_right).clicked() {
                                    self.set_hand_side(false);
                                }
                            });
                        });
                        ui.add_space(8.0);
                        let (rect, resp) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), 22.0),
                            egui::Sense::hover(),
                        );
                        ui.painter().text(
                            rect.left_center(),
                            egui::Align2::LEFT_CENTER,
                            "Tracking Range",
                            egui::FontId::proportional(13.0),
                            TEXT,
                        );
                        ui.painter().text(
                            rect.right_center(),
                            egui::Align2::RIGHT_CENTER,
                            ">",
                            egui::FontId::proportional(14.0),
                            MUTED,
                        );
                        resp.on_hover_text(
                            "The boundary box is a separate command set. It is not wired yet.",
                        );
                    });
            })
            .response
    }

    fn help_text(&self) -> String {
        format!(
            "preview {:.1} fps · camera {} · boot {} · SN {}",
            self.fps,
            self.cam_fps.map(|h| format!("{h:.1}")).unwrap_or_else(|| "…".into()),
            match self.boot {
                Some(true) => "ready",
                Some(false) => "no ack",
                None => "…",
            },
            if self.serial.is_empty() { "…" } else { &self.serial }
        )
    }

    fn preview_pane(&mut self, ui: &mut egui::Ui, size: egui::Vec2) {
        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        if !self.preview_open {
            ui.painter().rect_filled(rect, egui::CornerRadius::ZERO, egui::Color32::BLACK);
            return;
        }
        let Some(tex) = &self.tex else {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "waiting for the camera",
                egui::FontId::proportional(16.0),
                MUTED,
            );
            return;
        };
        let mut uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        if self.mirror {
            uv.min.x = 1.0;
            uv.max.x = 0.0;
        }
        let mut image = egui::Image::from_texture(tex)
            .fit_to_exact_size(size)
            .maintain_aspect_ratio(true)
            .uv(uv);
        if self.rotation % 4 != 0 {
            image = image.rotate(
                self.rotation as f32 * std::f32::consts::FRAC_PI_2,
                egui::vec2(0.5, 0.5),
            );
        }
        ui.put(rect, image);
    }
}

fn section(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    egui::CollapsingHeader::new(egui::RichText::new(title).size(14.0).strong())
        .default_open(true)
        .show(ui, body);
}

fn pill(ui: &mut egui::Ui, text: &str, on: bool) -> egui::Response {
    let fill = if on { RED } else { CARD };
    let color = if on { egui::Color32::WHITE } else { TEXT };
    ui.add(
        egui::Button::new(egui::RichText::new(text).color(color).size(13.0))
            .fill(fill)
            .corner_radius(egui::CornerRadius::same(8)),
    )
}

fn value_chip(ui: &mut egui::Ui, text: &str) {
    egui::Frame::new()
        .fill(CARD)
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(6)
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(12.0));
        });
}

fn switch(ui: &mut egui::Ui, on: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(42.0, 22.0), egui::Sense::click());
    let painter = ui.painter();
    let bg = if on { RED } else { egui::Color32::from_rgb(68, 68, 74) };
    painter.rect_filled(rect, egui::CornerRadius::same(11), bg);
    let cx = if on { rect.right() - 12.0 } else { rect.left() + 12.0 };
    painter.circle_filled(egui::pos2(cx, rect.center().y), 8.0, egui::Color32::WHITE);
    resp
}

fn switch_row(ui: &mut egui::Ui, label: &str, on: bool) -> bool {
    let mut hit = false;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).size(13.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if switch(ui, on).clicked() {
                hit = true;
            }
        });
    });
    hit
}

fn slider_row(ui: &mut egui::Ui, value: &mut i32, range: CtrlRange, suffix: &str) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().slider_width = (ui.available_width() - 78.0).max(40.0);
        // smart_aim stays on: a click on the knob steps toward the pointer.
        // With it off, that click jumps to the raw pixel and the value moves
        // the wrong way (sharpness 50→52, compensation 9→11).
        let resp = ui
            .add(
                egui::Slider::new(value, range.minimum..=range.maximum)
                    .step_by(range.step.max(1) as f64)
                    .show_value(false),
            )
            .on_hover_text("Double-click to reset to the camera default");
        if resp.double_clicked() {
            *value = range.default_value.clamp(range.minimum, range.maximum);
            changed = true;
        } else {
            changed = resp.changed();
        }
        value_chip(ui, &format!("{value}{suffix}"));
    });
    changed
}

/// Global / Face pills plus a cycle button: `0` and `1` map to Global and
/// Face per cameraSetAutoFocusModeR / cameraSetExposureModeR RE; the cycle
/// button sends 0..5 so the device (which echoes nothing on these GETs) is
/// settled by watching the preview instead of guessing.
///
/// Returns the mode that was sent. The caller has to store it — the pills
/// compare against that stored value, so a send that does not update it
/// leaves Face looking unselected.
fn face_global(
    ui: &mut egui::Ui,
    label: &str,
    cur: u32,
    tx: &std::sync::mpsc::Sender<UiMsg>,
    msg: fn(u32) -> UiMsg,
) -> Option<u32> {
    let mut picked = None;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).size(13.0).color(MUTED));
        if pill(ui, "Global", cur == 0).clicked() {
            picked = Some(0);
        }
        if pill(ui, "Face", cur == 1).clicked() {
            picked = Some(1);
        }
        value_chip(ui, &cur.to_string());
        if ui
            .small_button("\u{25B8}")
            .on_hover_text("cycle mode values 0..5 onto the wire; watch the preview")
            .clicked()
        {
            picked = Some((cur + 1) % 6);
        }
    });
    if let Some(mode) = picked {
        let _ = tx.send(msg(mode));
    }
    picked
}

fn swatch(ui: &mut egui::Ui, text: &str, on: bool) -> egui::Response {
    let fill = match text {
        "Dark" => egui::Color32::BLACK,
        "Room" => egui::Color32::from_rgb(90, 100, 114),
        _ => egui::Color32::WHITE,
    };
    let color = if text == "WOLF" { egui::Color32::from_rgb(210, 32, 48) } else { TEXT };
    let button = egui::Button::new(egui::RichText::new(text).color(color).size(11.0))
        .fill(fill)
        .corner_radius(egui::CornerRadius::same(6))
        .stroke(egui::Stroke::new(if on { 2.0 } else { 0.0 }, RED));
    ui.add(button)
}

fn icon_button(
    ui: &mut egui::Ui,
    tip: &str,
    on: bool,
    draw: fn(&egui::Painter, egui::Rect),
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(34.0, 30.0), egui::Sense::click());
    let fill = if on {
        RED
    } else if resp.hovered() {
        egui::Color32::from_rgb(58, 58, 64)
    } else {
        egui::Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, egui::CornerRadius::same(8), fill);
    draw(ui.painter(), rect);
    resp.on_hover_text(tip)
}

fn draw_caret(painter: &egui::Painter, at: egui::Pos2, dir: egui::Vec2, color: egui::Color32) {
    let side = egui::vec2(-dir.y, dir.x);
    let tip = at + dir * 6.0;
    let base = at - dir * 3.0;
    painter.add(egui::Shape::convex_polygon(
        vec![tip, base + side * 4.5, base - side * 4.5],
        color,
        egui::Stroke::NONE,
    ));
}

fn draw_rotate_ccw(painter: &egui::Painter, rect: egui::Rect) {
    draw_arc_arrow(painter, rect.center(), true);
}

fn draw_rotate_cw(painter: &egui::Painter, rect: egui::Rect) {
    draw_arc_arrow(painter, rect.center(), false);
}

fn draw_arc_arrow(painter: &egui::Painter, center: egui::Pos2, ccw: bool) {
    let radius = 8.0;
    let (a0, a1) = if ccw { (0.4, 4.6) } else { (-0.4, -4.6) };
    let mut pts = Vec::new();
    let steps = 14;
    for i in 0..=steps {
        let t = a0 + (a1 - a0) * i as f32 / steps as f32;
        pts.push(center + egui::vec2(t.cos(), t.sin()) * radius);
    }
    painter.add(egui::Shape::line(pts, egui::Stroke::new(1.6, TEXT)));
    let end = a1;
    let tip = center + egui::vec2(end.cos(), end.sin()) * radius;
    let tangent = if ccw {
        egui::vec2(-end.sin(), end.cos())
    } else {
        egui::vec2(end.sin(), -end.cos())
    };
    draw_caret(painter, tip, tangent.normalized(), TEXT);
}

fn draw_mirror(painter: &egui::Painter, rect: egui::Rect) {
    let c = rect.center();
    painter.line_segment(
        [c + egui::vec2(0.0, -8.0), c + egui::vec2(0.0, 8.0)],
        egui::Stroke::new(1.4, TEXT),
    );
    painter.add(egui::Shape::closed_line(
        vec![
            c + egui::vec2(-8.0, -6.0),
            c + egui::vec2(-2.0, -6.0),
            c + egui::vec2(-2.0, 6.0),
            c + egui::vec2(-8.0, 6.0),
        ],
        egui::Stroke::new(1.4, TEXT),
    ));
    painter.add(egui::Shape::closed_line(
        vec![
            c + egui::vec2(2.0, -4.0),
            c + egui::vec2(8.0, -4.0),
            c + egui::vec2(8.0, 4.0),
            c + egui::vec2(2.0, 4.0),
        ],
        egui::Stroke::new(1.4, TEXT),
    ));
}

fn draw_help(painter: &egui::Painter, rect: egui::Rect) {
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "?",
        egui::FontId::proportional(16.0),
        TEXT,
    );
}

fn draw_person(painter: &egui::Painter, rect: egui::Rect) {
    let c = rect.center();
    painter.circle_stroke(c + egui::vec2(0.0, -5.0), 3.2, egui::Stroke::new(1.5, TEXT));
    painter.add(egui::Shape::line(
        vec![
            c + egui::vec2(-6.0, 7.0),
            c + egui::vec2(-4.0, 1.0),
            c + egui::vec2(0.0, 3.0),
            c + egui::vec2(4.0, 1.0),
            c + egui::vec2(6.0, 7.0),
        ],
        egui::Stroke::new(1.5, TEXT),
    ));
}

fn paint_mode_fill(painter: &egui::Painter, rect: egui::Rect, on: bool, hovered: bool) {
    let fill = if on {
        RED
    } else if hovered {
        egui::Color32::from_rgb(58, 58, 64)
    } else {
        return;
    };
    painter.rect_filled(rect, egui::CornerRadius::same(8), fill);
}

fn mode_row(
    ui: &mut egui::Ui,
    width: f32,
    label: &str,
    on: bool,
    caret: bool,
    draw: fn(&egui::Painter, egui::Rect),
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, 26.0), egui::Sense::click());
    paint_mode_fill(ui.painter(), rect, on, resp.hovered());
    let icon = egui::Rect::from_min_size(rect.min, egui::vec2(28.0, rect.height()));
    draw(ui.painter(), icon);
    ui.painter().text(
        rect.left_center() + egui::vec2(28.0, 0.0),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(13.0),
        TEXT,
    );
    if caret {
        draw_chevron_down(ui.painter(), rect.left_center() + egui::vec2(72.0, 0.0));
    }
    resp
}

fn side_pill(ui: &mut egui::Ui, label: &str, on: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(54.0, 26.0), egui::Sense::click());
    let fill = if on { RED } else { CARD };
    ui.painter().rect_filled(rect, egui::CornerRadius::same(8), fill);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(12.0),
        TEXT,
    );
    resp
}

fn draw_chevron_down(painter: &egui::Painter, at: egui::Pos2) {
    let stroke = egui::Stroke::new(1.5, TEXT);
    painter.line_segment(
        [at + egui::vec2(-3.5, -2.0), at + egui::vec2(0.0, 2.0)],
        stroke,
    );
    painter.line_segment(
        [at + egui::vec2(0.0, 2.0), at + egui::vec2(3.5, -2.0)],
        stroke,
    );
}

fn draw_hand(painter: &egui::Painter, rect: egui::Rect) {
    let c = rect.center();
    let stroke = egui::Stroke::new(1.4, TEXT);
    painter.rect_stroke(
        egui::Rect::from_center_size(c + egui::vec2(1.0, 3.0), egui::vec2(10.0, 8.0)),
        egui::CornerRadius::same(3),
        stroke,
        egui::StrokeKind::Outside,
    );
    for i in 0..4 {
        let x = -3.5 + i as f32 * 3.0;
        painter.line_segment(
            [c + egui::vec2(x, -1.0), c + egui::vec2(x, -8.0)],
            stroke,
        );
    }
    painter.line_segment(
        [c + egui::vec2(-4.0, 2.0), c + egui::vec2(-8.0, -2.0)],
        stroke,
    );
}

fn draw_group(painter: &egui::Painter, rect: egui::Rect) {
    let c = rect.center();
    painter.circle_stroke(c + egui::vec2(-4.0, -4.0), 2.6, egui::Stroke::new(1.4, TEXT));
    painter.circle_stroke(c + egui::vec2(4.0, -5.0), 2.4, egui::Stroke::new(1.4, TEXT));
    painter.line_segment(
        [c + egui::vec2(-8.0, 7.0), c + egui::vec2(2.0, 7.0)],
        egui::Stroke::new(1.4, TEXT),
    );
    painter.line_segment(
        [c + egui::vec2(1.0, 6.0), c + egui::vec2(8.0, 6.0)],
        egui::Stroke::new(1.4, TEXT),
    );
}

fn draw_lock(painter: &egui::Painter, rect: egui::Rect) {
    let c = rect.center();
    painter.rect_stroke(
        egui::Rect::from_center_size(c + egui::vec2(0.0, 3.0), egui::vec2(12.0, 9.0)),
        egui::CornerRadius::same(2),
        egui::Stroke::new(1.4, TEXT),
        egui::StrokeKind::Outside,
    );
    painter.add(egui::Shape::line(
        vec![
            c + egui::vec2(-4.0, 0.0),
            c + egui::vec2(-4.0, -3.0),
            c + egui::vec2(0.0, -6.0),
            c + egui::vec2(4.0, -3.0),
            c + egui::vec2(4.0, 0.0),
        ],
        egui::Stroke::new(1.4, TEXT),
    ));
}

fn draw_record(painter: &egui::Painter, rect: egui::Rect) {
    painter.circle_filled(rect.center(), 6.0, RED);
}

fn draw_camera(painter: &egui::Painter, rect: egui::Rect) {
    let c = rect.center();
    painter.rect_stroke(
        egui::Rect::from_center_size(c, egui::vec2(14.0, 10.0)),
        egui::CornerRadius::same(2),
        egui::Stroke::new(1.4, TEXT),
        egui::StrokeKind::Outside,
    );
    painter.circle_stroke(c, 2.5, egui::Stroke::new(1.3, TEXT));
}

fn draw_folder(painter: &egui::Painter, rect: egui::Rect) {
    let c = rect.center();
    painter.add(egui::Shape::closed_line(
        vec![
            c + egui::vec2(-8.0, -2.0),
            c + egui::vec2(-8.0, 6.0),
            c + egui::vec2(8.0, 6.0),
            c + egui::vec2(8.0, -2.0),
            c + egui::vec2(2.0, -2.0),
            c + egui::vec2(0.0, -6.0),
            c + egui::vec2(-8.0, -6.0),
        ],
        egui::Stroke::new(1.4, TEXT),
    ));
}

fn draw_mic(painter: &egui::Painter, rect: egui::Rect) {
    let c = rect.center();
    painter.rect_stroke(
        egui::Rect::from_center_size(c + egui::vec2(0.0, -2.0), egui::vec2(6.0, 10.0)),
        egui::CornerRadius::same(3),
        egui::Stroke::new(1.4, TEXT),
        egui::StrokeKind::Outside,
    );
    painter.line_segment(
        [c + egui::vec2(0.0, 4.0), c + egui::vec2(0.0, 8.0)],
        egui::Stroke::new(1.4, TEXT),
    );
}

impl eframe::App for Panel {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.apply_theme(ctx);
        self.drain_events();
        self.update_texture(ctx);
        // Wake is sent on the control thread at startup. Give it a moment,
        // then recenter — sleep leaves the gimbal parked downward.
        if !self.opened && self.playback_epoch.elapsed() > Duration::from_millis(800) {
            self.opened = true;
            self.home_gimbal();
        }
        if self.recording && self.rec_tick.elapsed() >= Duration::from_millis(500) {
            self.rec_tick = Instant::now();
            self.save_frame();
        }

        egui::SidePanel::right("ctl")
            .exact_width(340.0)
            .frame(egui::Frame::new().fill(PANEL).inner_margin(12))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let image_on = self.tab == 0;
                    let more_on = self.tab == 1;
                    if tab_button(ui, "Image", image_on).clicked() {
                        self.tab = 0;
                    }
                    ui.add_space(28.0);
                    if tab_button(ui, "More", more_on).clicked() {
                        self.tab = 1;
                    }
                });
                ui.add_space(8.0);
                egui::ScrollArea::vertical().drag_to_scroll(false).show(ui, |ui| {
                    if self.tab == 0 {
                        self.image_tab(ui);
                    } else {
                        self.more_tab(ui);
                    }
                    ui.label(egui::RichText::new(&self.ai_line).size(11.0).color(MUTED));
                    if let Some(line) = self.log.last() {
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(line).size(11.0).color(MUTED));
                    }
                });
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG).inner_margin(10))
            .show(ctx, |ui| {
                self.preview_top(ui);
                ui.add_space(6.0);
                let bottom = 68.0;
                let size = egui::vec2(ui.available_width(), (ui.available_height() - bottom).max(80.0));
                self.preview_pane(ui, size);
                ui.add_space(6.0);
                self.preview_bottom(ui);
            });

        let elapsed = self.playback_epoch.elapsed().as_nanos() as u64;
        ctx.request_repaint_after(playback_delay(elapsed));
    }
}

fn tab_button(ui: &mut egui::Ui, text: &str, on: bool) -> egui::Response {
    let color = if on { TEXT } else { MUTED };
    let resp = ui.add(
        egui::Button::new(egui::RichText::new(text).size(16.0).color(color)).frame(false),
    );
    if on {
        let rect = resp.rect;
        ui.painter().hline(rect.x_range(), rect.bottom() + 3.0, egui::Stroke::new(2.0, RED));
    }
    resp
}

// ---- headless smoke mode ----------------------------------------------------

fn headless(shots: u32) -> Result<(), Box<dyn Error>> {
    let xu = V4lXu::open_camera()?;
    let mut d = ObsbotUsb::new(xu);
    println!("boot ack: {}", d.boot_status()?);
    println!("serial: {}", d.serial()?);
    println!("presets: {:?}", d.preset_names(3)?);
    let a = d.attitude()?;
    println!("attitude: {:.2} {:.2} {:.2}", a[0], a[1], a[2]);
    match d.request(&cmd::get_tally()) {
        Ok(f) => println!(
            "tally reply payload: {:02x?}",
            &f.payload[..f.payload_len.min(16) as usize]
        ),
        Err(_) => println!(
            "tally get: no wire reply (expected on model-1/2 devices; \
             libdev reads tally state from async 0x89 events instead)"
        ),
    }

    let mut cap = Capture::start_mjpeg_fps(1280, 720, 60)?;
    println!(
        "capture: {}x{} mjpeg @ {:.2} fps (interval {}/{})",
        cap.w, cap.h, cap.fps, cap.interval_num, cap.interval_den
    );
    let range = cap.query_ctrl(obsbot_core::linux_kernel::CID_ZOOM_ABSOLUTE)?;
    let now = cap.get_ctrl(obsbot_core::linux_kernel::CID_ZOOM_ABSOLUTE)?;
    println!(
        "zoom: {now} range {}..{} step {}",
        range.minimum, range.maximum, range.step
    );
    let mid = range.minimum + (range.maximum - range.minimum) / 2;
    cap.set_ctrl(obsbot_core::linux_kernel::CID_ZOOM_ABSOLUTE, mid)?;
    println!("zoom set to {mid} ok");
    cap.set_ctrl(obsbot_core::linux_kernel::CID_ZOOM_ABSOLUTE, now)?;

    std::fs::create_dir_all("/tmp/obsbot_panel")?;
    let t0 = Instant::now();
    let mut saved = 0;
    while saved < shots && t0.elapsed() < Duration::from_secs(20) {
        let (data, _ts) = cap.next_frame()?;
        let path = format!("/tmp/obsbot_panel/smoke_{saved}.jpg");
        std::fs::write(&path, &data)?;
        match jpeg_decoder::Decoder::new(&data[..]).decode() {
            Ok(px) => println!("frame {saved}: {} B jpeg, {} px -> {path}", data.len(), px.len()),
            Err(e) => println!("frame {saved}: {} B jpeg, decode err {e} -> {path}", data.len()),
        }
        saved += 1;
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args: Vec<String> = std::env::args().collect();
    if let Some(a) = args.iter().find(|a| a.starts_with("--shots=")) {
        let n: u32 = a.trim_start_matches("--shots=").parse().unwrap_or(5);
        return headless(n).map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
            format!("{e}").into()
        });
    }

    let (ev_tx, ev_rx) = mpsc::channel::<Ev>();
    let (cap_tx, cap_rx) = mpsc::channel::<CapCmd>();
    let (raw_tx, raw_rx) = mpsc::channel::<Vec<u8>>();
    let hold = Arc::new(StreamHold::new());
    let ui_tx = spawn_ctrl(ev_tx.clone(), Arc::clone(&hold));
    spawn_decode(ev_tx.clone(), raw_rx);
    spawn_motion(ev_tx.clone(), cap_rx);
    spawn_capture(ev_tx, raw_tx, hold);

    let app = Panel::new(ev_rx, ui_tx, cap_tx);
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 760.0])
            // OBSBOT Center clamps its window both ways; we must too. WSLg's
            // software GL (llvmpipe) gets disconnected by the compositor when
            // a resize configures a degenerate or enormous surface — a raw
            // "Connection reset by peer" kill instead of a graceful shrink.
            // The floor also keeps every tab's layout above the point where
            // the pill rows wrap into mush.
            .with_min_inner_size([1040.0, 620.0])
            .with_max_inner_size([1920.0, 1400.0])
            .with_title("OBSBOT Center"),
        // Vsync plus a 16 ms repaint delay presents at ~30 fps. The panel
        // paces itself at PLAYBACK_HZ instead.
        vsync: false,
        ..Default::default()
    };
    let res = eframe::run_native(
        "OBSBOT Center",
        opts,
        Box::new(move |_cc| Ok(Box::new(app))),
    );
    res.map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
        format!("eframe: {e}").into()
    })
}

#[cfg(test)]
mod tests {
    use super::{
        hand_gesture, playback_delay, scaled_stick, step_axis, stick_axes, summarize_ai_status,
        zoom_times, HAND_GESTURE_MS, PLAYBACK_STEP_NS,
    };

    #[test]
    fn hand_click_ignores_the_echo_that_cancels_it() {
        assert_eq!(hand_gesture(false, 0), Some(true));
        assert_eq!(hand_gesture(true, 0), None);
        assert_eq!(hand_gesture(true, HAND_GESTURE_MS - 1), None);
        assert_eq!(hand_gesture(true, HAND_GESTURE_MS), Some(false));
    }

    #[test]
    fn playback_delay_locks_to_60hz_grid() {
        let step = PLAYBACK_STEP_NS;
        assert_eq!(playback_delay(0).as_nanos() as u64, step);
        assert_eq!(playback_delay(step).as_nanos() as u64, step);
        assert_eq!(playback_delay(step / 2).as_nanos() as u64, step - step / 2);
        assert_eq!(playback_delay(step - 1).as_nanos() as u64, 1);
        assert_ne!(playback_delay(0), std::time::Duration::ZERO);
    }

    #[test]
    fn diagonal_stick_matches_cardinal_speed() {
        let (pan, tilt) = scaled_stick(0.0, -1.0, 0.4);
        assert_eq!(pan, 0.0);
        assert!((tilt + 0.4).abs() < 1e-6);
        let (pan, tilt) = scaled_stick(1.0, 1.0, 1.0);
        let mag = (pan * pan + tilt * tilt).sqrt();
        assert!((mag - 1.0).abs() < 1e-5, "{mag}");
        assert!((pan - tilt).abs() < 1e-5);
    }

    #[test]
    fn yaw_reverse_flips_only_pan() {
        let (pan, tilt) = stick_axes(1.0, 0.5, 1.0, false);
        let (rev_pan, rev_tilt) = stick_axes(1.0, 0.5, 1.0, true);
        assert!((rev_pan + pan).abs() < 1e-5);
        assert!((rev_tilt - tilt).abs() < 1e-5);
    }

    #[test]
    fn zoom_readout_matches_libdev_normalization() {
        // libdev cameraGetZoomAbsoluteR for product Tail2Lite (3): 1 + (cur-min)/(max-min)
        assert!((zoom_times(0, 0, 100) - 1.0).abs() < 1e-5);
        assert!((zoom_times(20, 0, 100) - 1.2).abs() < 1e-5);
        assert!((zoom_times(100, 0, 100) - 2.0).abs() < 1e-5);
        // A product reporting range 0..300 would run 1x..2x wide; arbitrary ranges
        // still normalize to the span.
        assert!((zoom_times(150, 0, 300) - 1.5).abs() < 1e-5);
    }

    #[test]
    fn preset_and_inverted_fields_match_the_handoff() {
        let add = obsbot_core::cmd::preset_add_lite(2, 10.0, -4.0, 1.5);
        assert_eq!(add.cmd_pair(), [0x37, 0x30]);
        assert_eq!(add.to_payload().len(), 24);
        let name = obsbot_core::cmd::preset_rename(2, "P2");
        assert_eq!(name.cmd_pair(), [0x3D, 0x30]);
        assert_eq!(&name.to_payload()[..4], &2u32.to_le_bytes());
        assert_eq!(name.to_payload().last().copied(), Some(0));
        assert_eq!(obsbot_core::cmd::bg_disabled_byte(true), 0);
        assert_eq!(obsbot_core::cmd::bg_disabled_byte(false), 1);
        assert_eq!(obsbot_core::cmd::mic_enabled_byte(true), 0);
        assert_eq!(obsbot_core::cmd::mic_enabled_byte(false), 1);
        assert_eq!(obsbot_core::cmd::yaw_reverse(true).cmd_pair(), [0x65, 0x30]);
        assert_eq!(obsbot_core::cmd::hand_track_mode(1).cmd_pair(), [0x49, 0x30]);
        assert_eq!(obsbot_core::cmd::track_speed(2).cmd_pair(), [0x68, 0x30]);
        assert_eq!(obsbot_core::cmd::audio_volume(40).cmd_pair(), [0x74, 0x10]);
        assert_eq!(obsbot_core::cmd::get_ai_status().cmd_pair(), [0x13, 0x30]);
        let line = summarize_ai_status(&[1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 128, 63]);
        assert!(line.contains("10100"), "{line}");
    }

    #[test]
    fn gimbal_steps_one_degree_at_a_time() {
        // A full second of travel must not be applied in one jump.
        let (pos, frac) = step_axis(0, 0.0, 45.0, 1.0, 3600, -468_000, 468_000);
        assert_eq!(pos, 3600);
        assert!((frac - 44.0 * 3600.0).abs() < 1.0, "{frac}");
        // 1/45 s at 45°/s is exactly one degree.
        let (pos, frac) = step_axis(0, 0.0, 45.0, 1.0 / 45.0, 3600, -468_000, 468_000);
        assert_eq!(pos, 3600);
        assert!(frac.abs() < 1.0, "{frac}");
        // Two half-steps accumulate into one degree, and the limit holds.
        let (pos, frac) = step_axis(0, 0.0, -45.0, 1.0 / 90.0, 3600, -324_000, 324_000);
        assert_eq!(pos, 0);
        let (pos, _) = step_axis(pos, frac, -45.0, 1.0 / 90.0, 3600, -324_000, 324_000);
        assert_eq!(pos, -3600);
        let (pos, _) = step_axis(468_000, 0.0, 45.0, 1.0, 3600, -468_000, 468_000);
        assert_eq!(pos, 468_000);
    }
}
