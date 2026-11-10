// Live AiMode probe: read XU selector 6 blob, sweep single-byte flips, restore.
// Run: sudo cargo run --features codec --example probe_aimode
use obsbot_core::device::ObsbotUsb;
use obsbot_core::linux_kernel::V4lXu;
use obsbot_core::XuTransport;
use std::thread::sleep;
use std::time::Duration;

fn attitude(d: &mut ObsbotUsb<V4lXu>) -> [f32; 3] {
    match d.attitude() {
        Ok(a) => a,
        Err(_) => [0.0; 3],
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let xu = V4lXu::open_camera()?;
    let mut d = ObsbotUsb::new(xu);
    println!("serial: {}", d.serial()?);

    let len = d.transport_mut().get_len(6).unwrap_or(60);
    let base = d.transport_mut().get_cur(6, len as usize)?;
    println!("AiMode len={len} base={}", hex(&base));

    let mut best = (usize::MAX, base.clone());
    // Sweep: flip each zero byte in 1..=min(len-1, 0x28) to 1, send, watch.
    for i in 1..base.len().min(0x28) {
        if base[i] != 0 {
            continue;
        }
        let mut v = base.clone();
        v[i] = 1;
        d.transport_mut().set_cur(6, &v)?;
        println!("try byte {i:#04x}=01 sent");
        sleep(Duration::from_millis(2500));
        let a = attitude(&mut d);
        if a[0] != 0.0 || a[1] != 0.0 {
            println!("  >>> attitude moved: {a:?}");
            best = (i, v.clone());
            sleep(Duration::from_millis(4000));
            break;
        }
    }
    // Restore original blob.
    d.transport_mut().set_cur(6, &base)?;
    println!("restored. hot byte idx={:#04x?}", best.0);
    if best.0 != usize::MAX {
        println!("hot payload: {}", hex(&best.1));
    }
    Ok(())
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ")
}
