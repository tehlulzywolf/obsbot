// Timed AI-tracking live test: attitude baseline -> AI ON for 15s (track
// the user) -> AI OFF -> attitude again. Ground truth is physical: during
// the ON window the gimbal should pan/tilt to follow whoever sits in front
// of the camera. Run: sudo cargo run --features codec --example test_ai
use std::thread::sleep;
use std::time::Duration;

use obsbot_core::cmd;
use obsbot_core::device::ObsbotUsb;
use obsbot_core::linux_kernel::V4lXu;

fn att(d: &mut ObsbotUsb<V4lXu>, label: &str) {
    for i in 0..5 {
        match d.attitude() {
            Ok([r, p, y]) => println!("attitude[{i}] {label}: r={r:.1} p={p:.1} y={y:.1}"),
            Err(e) => println!("attitude[{i}] {label}: ERR {e}"),
        }
        sleep(Duration::from_millis(1200));
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let xu = V4lXu::open_camera()?;
    let mut d = ObsbotUsb::new(xu);
    println!("serial: {}", d.serial()?);

    println!("=== baseline (AI untouched) ===");
    att(&mut d, "base");

    println!("=== AI ON: sit in front and wave/move for 12s ===");
    d.tracking_enable(true)?;
    d.command(&cmd::tracking_type(1))?; // person tracking
    sleep(Duration::from_millis(1500));
    att(&mut d, "ai-on");

    println!("=== AI OFF: gimbal should stop/freeze ===");
    d.tracking_enable(false)?;
    sleep(Duration::from_millis(1000));
    att(&mut d, "ai-off");
    Ok(())
}
