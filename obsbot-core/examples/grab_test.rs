// grab_test — headless proof that userspace UVC streaming works on the
// Tiny 2 Lite: control transport first, then VS probe/commit + bulk grab,
// save the first MJPEG frames as .jpg files.
// OBSBOT_NOCTRL=1 skips opening the VC control transport (isolation test).
use obsbot_core::device::ObsbotUsb;
use obsbot_core::stream::{Stream, MJPEG_720};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut ctrl_dev: Option<nusb::Device> = None;
    let dev = if std::env::var("OBSBOT_NOCTRL").is_ok() {
        println!("skipping control (OBSBOT_NOCTRL)");
        None
    } else {
        let t = obsbot_core::transport::NusbXu::open_camera()?;
        let dv = t.device(); // shared handle (usbfs rights are per-fd)
        let mut d = ObsbotUsb::new(t);
        let sn = d.serial()?;
        println!("serial: {sn}");
        ctrl_dev = Some(dv);
        Some(d)
    };

    println!("starting MJPEG 1280x720@30 stream...");
    let st = match ctrl_dev {
        Some(dv) => Stream::start_on(dv, MJPEG_720, 1280, 720)?,
        None => Stream::open_fresh(MJPEG_720, 1280, 720)?,
    };
    let t0 = Instant::now();
    let mut got = 0;
    let mut timeouts = 0;
    while got < 5 && t0.elapsed().as_secs() < 15 {
        match st.recv_timeout(std::time::Duration::from_secs(3)) {
            Ok(Some(f)) => {
                let ms = t0.elapsed().as_millis();
                println!(
                    "frame {got}: {}x{} rgba={}B mjpeg={}B at {ms}ms",
                    f.w,
                    f.h,
                    f.rgba.len(),
                    f.mjpeg.as_ref().map_or(0, |v| v.len())
                );
                if got == 0 {
                    if let Some(j) = &f.mjpeg {
                        if std::fs::write("/tmp/grab0.jpg", j).is_ok() {
                            println!("saved /tmp/grab0.jpg");
                        }
                    }
                }
                got += 1;
            }
            Ok(None) => {
                timeouts += 1;
                println!("  no frame yet ({timeouts}x3s)");
            }
            Err(e) => {
                println!("recv error: {e}");
                break;
            }
        }
    }
    let dt = t0.elapsed().as_secs_f32();
    println!("{got} frames in {dt:.2}s = {:.1} fps", got as f32 / dt);
    drop(st);
    drop(dev);
    Ok(())
}
