// Kernel-path proof: XU control over UVCIOC_CTRL_QUERY + V4L2 mmap video,
// both on the uvcvideo node (no nusb, no raw-USB negotiation).
// Run (WSL, uvcvideo bound to the camera): sudo cargo run --features codec --example kernel_test
#![cfg(target_os = "linux")]

use obsbot_core::device::ObsbotUsb;
use obsbot_core::linux_kernel::{Capture, V4lXu};
use std::time::{Duration, Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // --- control: serial + boot + attitude over the kernel XU passthrough ---
    let xu = V4lXu::open_camera()?;
    let mut d = ObsbotUsb::new(xu);
    println!("serial: {}", d.serial()?);
    println!("boot_status acked: {}", d.boot_status()?);
    let a = d.attitude()?;
    println!("attitude: roll={:.2} pitch={:.2} yaw={:.2}", a[0], a[1], a[2]);

    // --- video: MJPEG 1280x720 via V4L2 ---
    println!("starting V4L2 MJPEG 1280x720...");
    let mut cap = Capture::start_mjpeg(1280, 720)?;
    println!("negotiated {}x{} mjpeg={}", cap.w, cap.h, cap.mjpeg);

    let t0 = Instant::now();
    let mut saved = 0;
    while saved < 5 && t0.elapsed() < Duration::from_secs(15) {
        let (payload, ts) = cap.next_frame()?;
        if !cap.mjpeg {
            println!("frame {saved}: {}B yuy2 ts={ts}", payload.len());
            saved += 1;
            continue;
        }
        // sanity: MJPEG payloads start with SOI
        if payload.starts_with(&[0xFF, 0xD8]) {
            let path = format!("/tmp/obsbot_f{saved}.jpg");
            std::fs::write(&path, &payload)?;
            match jpeg_decoder::Decoder::new(&payload[..]).decode() {
                Ok(pixels) => println!(
                    "frame {saved}: {}B jpeg -> {} raw px -> {path}",
                    payload.len(),
                    pixels.len()
                ),
                Err(e) => println!("frame {saved}: {}B jpeg, decode err {e} (still saved)", payload.len()),
            }
            saved += 1;
        } else {
            println!("frame {saved}: payload not JPEG: {:02x?}", &payload[..8.min(payload.len())]);
        }
    }
    println!("done: {saved} frames in {:.1?}", t0.elapsed().as_secs_f64());
    Ok(())
}
