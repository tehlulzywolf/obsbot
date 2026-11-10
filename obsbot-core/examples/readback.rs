// readback decode: dump the 60-byte AiMode GET_CUR buffer and map it against
// the known repro3 writes to decode the struct layout (esp. 0x0216 offsets).
// Run as root in WSL: cargo run --features codec --example readback
use obsbot_core::device::{ObsbotUsb, XuTransport};
use obsbot_core::linux_kernel::V4lXu;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let xu = V4lXu::open_camera()?;
    let mut d = ObsbotUsb::new(xu);
    println!("serial: {}", d.serial()?);
    let n = d.transport_mut().get_len(6)?;
    let b = d.transport_mut().get_cur(6, 60)?;
    println!("get_len -> {n}, raw ({} bytes):", b.len());
    for (i, chunk) in b.chunks(16).enumerate() {
        print!("{:04x}: ", i * 16);
        for x in chunk {
            print!("{:02x} ", x);
        }
        println!();
    }
    Ok(())
}
