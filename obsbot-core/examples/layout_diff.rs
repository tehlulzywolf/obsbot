// Differential layout decode for selector-6 AiMode state:
// read -> write sentinels into 5 known fields -> read -> byte diff.
// Moving bytes name each field's offset in the packed GET_CUR buffer.
// Sentinels are restored to sane values at the end.
// Run as root in WSL: cargo run --features codec --example layout_diff
use obsbot_core::device::{ObsbotUsb, XuTransport};
use obsbot_core::linux_kernel::V4lXu;
use std::thread::sleep;
use std::time::Duration;

fn dump(d: &mut ObsbotUsb<obsbot_core::linux_kernel::V4lXu>) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    Ok(d.transport_mut().get_cur(6, 60)?)
}

fn hexdump(b: &[u8]) {
    for (i, chunk) in b.chunks(16).enumerate() {
        print!("{:04x}: ", i * 16);
        for x in chunk {
            print!("{:02x} ", x);
        }
        println!();
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let xu = V4lXu::open_camera()?;
    let mut d = ObsbotUsb::new(xu);
    println!("serial: {}", d.serial()?);
    let a = dump(&mut d)?;
    println!("=== BEFORE ===");
    hexdump(&a);

    // Sentinels, one distinctive byte per field (restored below).
    d.ai_field(0x0112, &[0x11])?;
    sleep(Duration::from_millis(120));
    d.ai_field(0x0117, &[0x22])?;
    sleep(Duration::from_millis(120));
    d.ai_field(0x011C, &[0x33])?;
    sleep(Duration::from_millis(120));
    d.ai_field(0x0216, &[0x44, 0x55])?;
    sleep(Duration::from_millis(120));
    d.ai_field(0x1021, &[0x66, 0, 0, 0, 0x77, 0, 0, 0, 0x88, 0, 0, 0, 0x99, 0, 0, 0])?;
    sleep(Duration::from_millis(120));

    let b = dump(&mut d)?;
    println!("=== AFTER ===");
    hexdump(&b);
    println!("=== DIFF (offset: before -> after) ===");
    for i in 0..a.len().min(b.len()) {
        if a[i] != b[i] {
            println!("  {i:02x}: {:02x} -> {:02x}", a[i], b[i]);
        }
    }

    // Restore sane values (as repro3 left them).
    d.ai_field(0x0112, &[1])?;
    sleep(Duration::from_millis(120));
    d.ai_field(0x0117, &[1])?;
    sleep(Duration::from_millis(120));
    d.ai_field(0x011C, &[1])?;
    sleep(Duration::from_millis(120));
    d.ai_field(0x0216, &[0, 1])?;
    sleep(Duration::from_millis(120));
    d.ai_field(0x1021, &[1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0])?;
    sleep(Duration::from_millis(120));
    let c = dump(&mut d)?;
    println!("=== RESTORED ===");
    hexdump(&c);
    println!("END ok");
    Ok(())
}
