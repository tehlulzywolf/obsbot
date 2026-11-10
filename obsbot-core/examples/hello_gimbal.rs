//! Smoke demo against a live Tiny 2 Lite over the UVC-XU vendor channel:
//! attitude, preset names, a short gimbal nudge, stop, boot status.
//!
//! Run on Linux with the camera attached (WSL: `usbipd attach --wsl`):
//!   cargo run --features uvc --example hello_gimbal
//! NOTE: claims the VideoControl interface (webcam picture pauses while
//! this runs; uvcvideo re-attaches on exit).

#[cfg(all(target_os = "linux", feature = "uvc"))]
fn main() -> Result<(), String> {
    use obsbot_core::transport::NusbXu;
    use obsbot_core::ObsbotUsb;

    let xu = NusbXu::open_camera()?;
    let mut cam = ObsbotUsb::new(xu);

    println!("-- events (idle) --");
    for f in cam.poll_events() {
        println!("  event type={:02X} cmd={:02X}{:02X} ok={}", f.frame_type, f.cmd[0], f.cmd[1], f.crc_ok);
    }

    println!("-- attitude (cmd pair 08 18) --");
    match cam.attitude() {
        Ok([roll, pitch, yaw]) => println!("  roll={roll:.2} pitch={pitch:.2} yaw={yaw:.2}"),
        Err(e) => println!("  ERR {e:?}"),
    }

    println!("-- serial (cmd pair C8 18) --");
    match cam.serial() {
        Ok(s) => println!("  {s}"),
        Err(e) => println!("  ERR {e:?}"),
    }

    println!("-- preset names (cmd pair 04 3B, type 0x21, idx 0..2) --");
    match cam.preset_names(3) {
        Ok(v) => println!("  {v:?}"),
        Err(e) => println!("  ERR {e:?}"),
    }

    println!("-- nudge: speed 0.3 pan for 400 ms, then stop --");
    cam.gimbal_speed(0.3, 0.0, 0.0).map_err(|e| e.to_string())?;
    std::thread::sleep(std::time::Duration::from_millis(400));
    cam.gimbal_stop().map_err(|e| e.to_string())?;

    println!("-- boot status --");
    match cam.boot_status() {
        Ok(_) => println!("  acked (camera present & responsive)"),
        Err(e) => println!("  ERR {e:?}"),
    }

    println!("-- events (post) --");
    for f in cam.poll_events() {
        println!("  event type={:02X} cmd={:02X}{:02X} ok={} payload={}", f.frame_type, f.cmd[0], f.cmd[1], f.crc_ok,
            f.payload.iter().take(12).map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" "));
    }
    Ok(())
}

#[cfg(not(all(target_os = "linux", feature = "uvc")))]
fn main() {
    println!("live XU transport is Linux + `uvc` feature; codec tests run everywhere (`cargo test`).");
}
