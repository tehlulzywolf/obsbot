// sel6 second-write repro: does a GET_CUR pull (the pattern Center follows on
// every XU transaction in cap1.pcap, but our ai_field never did) unblock the
// "second selector-6 SET kills the usbip link" bug? Ordered so the most
// informative answer comes before the first fatal blow. Panel must be stopped
// and the supervisor paused. Run (as root in WSL):
//   cargo run --features codec --example repro_sel6
use obsbot_core::device::{ObsbotUsb, XuTransport};
use obsbot_core::linux_kernel::V4lXu;
use std::thread::sleep;
use std::time::Duration;

fn step(tag: &str, r: obsbot_core::device::Result<usize>) {
    match r {
        Ok(n) => println!("{tag}: OK ({n} bytes)"),
        Err(e) => println!("{tag}: ERR {e}"),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("MARK repro_sel6 start t={} (link kill = later lines stop + ENODEV)", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?.as_secs());
    let xu = V4lXu::open_camera()?;
    let mut d = ObsbotUsb::new(xu);
    println!("serial: {}", d.serial()?);

    let w = |d: &mut ObsbotUsb<V4lXu>, tag: &str| {
        println!("{tag}: write-> {:?}", d.ai_field(0x0112, &[0]).map(|_| "OK"));
    };
    let p = |d: &mut ObsbotUsb<V4lXu>, tag: &str| {
        let n = d.transport_mut().get_len(6);
        println!("{tag}: get_len-> {n:?}");
        let r: obsbot_core::device::Result<Vec<u8>> = d.transport_mut().get_cur(6, 60);
        println!("{tag}: get_cur-> {:?}", r.map(|v: Vec<u8>| (v.len(), format!("{:02x?}", &v[..4.min(v.len())]))));
    };

    w(&mut d, "W1");
    sleep(Duration::from_millis(100));
    p(&mut d, "P1");
    sleep(Duration::from_millis(100));
    w(&mut d, "W2");
    sleep(Duration::from_millis(100));
    p(&mut d, "P2");
    sleep(Duration::from_millis(100));
    w(&mut d, "W3");
    sleep(Duration::from_millis(100));
    // Fourth: a plain AA command to prove the AA path is still healthy
    // even if the sel6 experiments hurt the link.
    println!("AA serial probe-> {:?}", d.serial().map(|s| s.len()));
    println!("END survived");
    Ok(())
}
