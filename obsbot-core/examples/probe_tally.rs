// Live-probe tally commands with the REA-corrected encodings on the kernel
// XU path. Sets use pb tags (field3=light, field4=brightness); the GET
// carries pb fields {2,4,5}. libdev never waits on sets — we still check
// whether the device status-ACKs the pair.
// Run: sudo cargo run --features codec --example probe_tally
use std::thread::sleep;
use std::time::Duration;

use obsbot_core::cmd::{self, kind, set, Req};
use obsbot_core::device::ObsbotUsb;
use obsbot_core::linux_kernel::V4lXu;

fn try_req(d: &mut ObsbotUsb<V4lXu>, label: &str, r: &Req) -> bool {
    match d.request(r) {
        Ok(f) => {
            let n = f.payload_len.min(16) as usize;
            println!("ACK {label}: ftype={:02x} payload[{n}]={:02x?}", f.frame_type, &f.payload[..n]);
            true
        }
        Err(e) => {
            println!("TIMEOUT {label}: {e}");
            false
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let xu = V4lXu::open_camera()?;
    let mut d = ObsbotUsb::new(xu);
    println!("serial: {}", d.serial()?);

    println!("--- sets (watch the tally ring while this runs) ---");
    try_req(&mut d, "light ON  A0.10 pb[18 01]", &cmd::tally_light(true));
    sleep(Duration::from_millis(2500));
    try_req(&mut d, "bright 5  A0.10 pb[20 05]", &cmd::tally_brightness(5));
    sleep(Duration::from_millis(2500));
    try_req(&mut d, "bright 100 A0.10 pb[20 64]", &cmd::tally_brightness(100));
    sleep(Duration::from_millis(2500));
    try_req(&mut d, "light OFF A0.10 pb[18 00]", &cmd::tally_light(false));

    println!("--- get (may be model-gated silent on Tiny 2 Lite) ---");
    try_req(&mut d, "get A1.10 pb{2,4,5}", &cmd::get_tally());
    Ok(())
}
