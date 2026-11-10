// Live AI-command probe: try REA-derived class-A frames for aiSetEnabled /
// workmode / tracking, print wire ACKs, and check attitude deltas to see
// whether the camera actually starts following.
// Run: sudo cargo run --features codec --example probe_ai
use std::thread::sleep;
use std::time::Duration;

use obsbot_core::cmd;
use obsbot_core::device::ObsbotUsb;
use obsbot_core::linux_kernel::V4lXu;

fn attitude(d: &mut ObsbotUsb<V4lXu>) -> (f32, f32, f32) {
    match d.attitude() {
        Ok(a) => (a[0], a[1], a[2]),
        Err(_) => (0.0, 0.0, 0.0),
    }
}

fn try_req(d: &mut ObsbotUsb<V4lXu>, label: &str, r: &cmd::Req) {
    match d.request(r) {
        Ok(f) => {
            let n = f.payload_len.min(16) as usize;
            println!("ACK {label}: ftype={:02x} payload[{n}]={:02x?}", f.frame_type, &f.payload[..n]);
        }
        Err(e) => println!("TIMEOUT {label}: {e}"),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let xu = V4lXu::open_camera()?;
    let mut d = ObsbotUsb::new(xu);
    println!("serial: {}", d.serial()?);

    // candidate wire pairs from libdev cmd words: word 0x300XX -> [XX, 03]
    // gen-0: ai_on 0d03 (arg1=on,arg2=0)  newer: ai_on 6203 (arg0=on)
    // workmode 6303 (arg0=main,arg1=sub,arg2=0)  tracking_mode 1103 (arg0=mode)
    let mk = |pair: [u8; 2], ftype: u8, args: &[u8]| -> cmd::Req {
        cmd::Req::new(cmd::kind::SET, 0, 0, 0)
            .pair(pair)
            .ftype(ftype)
            .flags(0x0d)
            .args(args.to_vec())
    };

    let a0 = attitude(&mut d);
    println!("attitude pre: {a0:?}");

    try_req(&mut d, "ai_on.0d03.f3", &mk([0x0d, 0x03], 3, &[1, 0]));
    sleep(Duration::from_millis(500));
    try_req(&mut d, "ai_on.6203.f3", &mk([0x62, 0x03], 3, &[1]));
    try_req(&mut d, "workmode.6303.f3", &mk([0x63, 0x03], 3, &[1, 1, 0]));
    try_req(&mut d, "trackmode.1103.f3", &mk([0x11, 0x03], 3, &[1]));

    println!(">>> watch gimbal for 12s: move around <<<");
    for _ in 0..6 {
        sleep(Duration::from_millis(2000));
        let a = attitude(&mut d);
        println!("attitude: {a:?}");
    }

    // same candidates as class-A ftype 1 (in case device only parses f1)
    try_req(&mut d, "ai_on.0d03.f1", &mk([0x0d, 0x03], 1, &[1, 0]));
    try_req(&mut d, "ai_on.6203.f1", &mk([0x62, 0x03], 1, &[1]));
    println!(">>> watch gimbal for 8s: move around <<<");
    for _ in 0..4 {
        sleep(Duration::from_millis(2000));
        let a = attitude(&mut d);
        println!("attitude: {a:?}");
    }

    // turn candidates off
    try_req(&mut d, "ai_off.0d03.f3", &mk([0x0d, 0x03], 3, &[0, 0]));
    try_req(&mut d, "ai_off.6203.f3", &mk([0x62, 0x03], 3, &[0]));
    Ok(())
}
