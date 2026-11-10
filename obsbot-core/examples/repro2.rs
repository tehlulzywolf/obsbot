// repro2: reproduce the OTHER AI's exact kill conditions — MJPEG stream ACTIVE
// (like panel preview) + bare ai_field writes (no GET_CUR pull, like their
// panel does). If W2/W3 die here, the killer is "sel6 SET while streaming",
// and the fix is stream-suspend around sel6 writes (or AA pairs).
// Run as root in WSL, panel stopped: cargo run --features codec --example repro2
use obsbot_core::device::{ObsbotUsb, XuTransport};
use obsbot_core::linux_kernel::{Capture, V4lXu};
use std::thread::sleep;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?;
    println!("MARK repro2 start epoch={}", t.as_secs());
    let xu = V4lXu::open_camera()?;
    let mut d = ObsbotUsb::new(xu);
    println!("serial: {}", d.serial()?);

    // Start the stream first (panel-like), drain frames to prove it's live.
    let mut cap = Capture::start_mjpeg_fps(1280, 720, 60)?;
    let mut frames = 0;
    let deadline = std::time::Instant::now() + Duration::from_secs(4);
    while frames < 20 && std::time::Instant::now() < deadline {
        if cap.next_frame().is_ok() {
            frames += 1;
        }
    }
    println!("STREAM frames={frames} (want >=20)");

    // Their exact pattern: bare writes back-to-back, no pull, no delay,
    // while the stream runs.
    for i in 1..=3 {
        let r = d.ai_field(0x0112, &[0]);
        println!("W{i} (streaming)-> {:?}", r.map(|_| "OK"));
        sleep(Duration::from_millis(150));
        // keep draining so the stream stays alive through the writes
        let _ = cap.next_frame();
    }
    let alive = cap.next_frame().is_ok();
    println!("stream still delivering frames after writes: {alive}");
    println!("AA serial probe-> {:?}", d.serial().map(|s| s.len()));
    println!("END survived");
    Ok(())
}
