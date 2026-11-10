// Roadmap items 1-3 live probe: sweep face-focus, AF mode, exposure mode,
// mirror/flip, HDR and FaceAE with GET readback; print exactly which values
// the device accepts/echoes. Watch the panel preview for visual changes
// during the HDR / mirror sections (probe prints a countdown marker first).
// Run (panel must be stopped): sudo cargo run --features codec --example probe_items
use obsbot_core::device::ObsbotUsb;
use obsbot_core::linux_kernel::{CtrlNode, V4lXu, CID_ZOOM_ABSOLUTE};
use std::thread::sleep;
use std::time::Duration;

fn set_get<T, E: std::fmt::Display>(what: &str, r: Result<T, E>) -> Option<T> {
    match r {
        Ok(v) => Some(v),
        Err(e) => {
            println!("  {what}: ERR {e}");
            None
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let xu = V4lXu::open_camera()?;
    let mut d = ObsbotUsb::new(xu);
    println!("serial: {}", d.serial()?);

    // item 9: zoom ctrl range + live ratio (libdev formula: 1 + (cur-min)/(max-min))
    if let Ok(c) = CtrlNode::open() {
        if let Ok(r) = c.query_ctrl(CID_ZOOM_ABSOLUTE) {
            let cur = c.get_ctrl(CID_ZOOM_ABSOLUTE).unwrap_or(r.default_value);
            let span = (r.maximum - r.minimum).max(1) as f32;
            println!(
                "ZOOM ctrl: min={} max={} step={} def={} cur={} -> ratio {:.2}",
                r.minimum,
                r.maximum,
                r.step,
                r.default_value,
                cur,
                1.0 + (cur - r.minimum) as f32 / span
            );
        } else {
            println!("ZOOM ctrl: QUERYCTRL failed");
        }
    }

    // ---- item 1: AF mode (5F set / 5E get) ----
    let base_af = set_get("af get", d.af_mode()).unwrap_or(0);
    println!("AF mode baseline: {base_af}");
    for m in 0..=3u32 {
        let _ = d.set_af_mode(m);
        sleep(Duration::from_millis(400));
        match set_get("af get", d.af_mode()) {
            Some(v) => println!("AF set {m} -> device now reports {v}"),
            None => println!("AF set {m} -> readback FAILED"),
        }
    }
    let _ = d.set_af_mode(base_af);

    // ---- item 1: exposure mode (47 set / 46 get) ----
    let base_exp = set_get("exp get", d.exp_mode()).unwrap_or(0);
    println!("EXP mode baseline: {base_exp}");
    for m in 0..=3u32 {
        let _ = d.set_exp_mode(m);
        sleep(Duration::from_millis(400));
        match set_get("exp get", d.exp_mode()) {
            Some(v) => println!("EXP set {m} -> device now reports {v}"),
            None => println!("EXP set {m} -> readback FAILED"),
        }
    }
    let _ = d.set_exp_mode(base_exp);

    // ---- item 1: face focus (03 10, no getter — ack only) ----
    match d.face_focus(true) {
        Ok(()) => println!("FACE_FOCUS on: acked"),
        Err(e) => println!("FACE_FOCUS on: ERR {e}"),
    }
    match d.face_focus(false) {
        Ok(()) => println!("FACE_FOCUS off: acked"),
        Err(e) => println!("FACE_FOCUS off: ERR {e}"),
    }

    // ---- item 3: mirror/flip (2E set / 2D get) ----
    let base_mf = set_get("mirror get", d.mirror_flip()).unwrap_or(0);
    println!("MIRROR baseline: {base_mf}   (watch preview)");
    for v in 0..=3u32 {
        let _ = d.set_mirror_flip(v);
        sleep(Duration::from_millis(400));
        match set_get("mirror get", d.mirror_flip()) {
            Some(r) => println!("MIRROR set {v} -> device reports {r}"),
            None => println!("MIRROR set {v} -> readback FAILED"),
        }
        sleep(Duration::from_millis(600));
    }
    let _ = d.set_mirror_flip(base_mf);

    // ---- item 2: HDR — try sel6 field 0101 first, then AA pair 14 10 ----
    println!("HDR test A: AiMode sel6 field 0101 = 1  (watch preview ~3 s)");
    match d.hdr(true, false) {
        Ok(()) => println!("  sel6 0101=1 sent"),
        Err(e) => println!("  sel6 0101=1 ERR {e}"),
    }
    sleep(Duration::from_millis(3000));
    println!("HDR test B: AA pair 14 10 value 1  (watch preview ~3 s)");
    match d.hdr(true, true) {
        Ok(()) => println!("  AA 14 10=1 acked"),
        Err(e) => println!("  AA 14 10=1 ERR {e}"),
    }
    sleep(Duration::from_millis(3000));
    let _ = d.hdr(false, false);
    let _ = d.hdr(false, true);

    // ---- item 1: FaceAE via sel6 field 0103 (no getter; visual) ----
    println!("FACE_AE: sel6 0103=1 on (watch exposure behaviour ~3 s)");
    match d.ai_field(0x0103, &[1]) {
        Ok(()) => println!("  sent on"),
        Err(e) => println!("  ERR {e}"),
    }
    sleep(Duration::from_millis(3000));
    match d.ai_field(0x0103, &[0]) {
        Ok(()) => println!("  FACE_AE off sent"),
        Err(e) => println!("  off ERR {e}"),
    }

    println!("done");
    Ok(())
}
