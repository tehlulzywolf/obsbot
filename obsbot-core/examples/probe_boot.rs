//! boot_status pair probe, repeatability edition. Live finding: pair
//! "17 10" returns type-09 echo with a 2-byte payload; bogus pairs return
//! len=0. Run 3 rounds with a bogus control to confirm distinctness.
//! cargo run --features uvc --example probe_boot

#[cfg(all(target_os = "linux", feature = "uvc"))]
fn main() -> Result<(), String> {
    use obsbot_core::device::XuTransport;
    use obsbot_core::frame;
    use obsbot_core::transport::NusbXu;

    let xu = NusbXu::open_camera()?;
    let mut cam = obsbot_core::ObsbotUsb::new(xu);
    let t = cam.transport_mut();
    t.get_len(2).map_err(|e| e.to_string())?;

    let mut seq = 0x50u8;
    for round in 0..3 {
        for (label, pair, flags) in [
            ("ctrl-bogus eeee", [0xeeu8, 0xee], 0x0du8),
            ("boot 1710 flags0d", [0x17u8, 0x10], 0x0d),
            ("boot 1710 flags04", [0x17, 0x10], 0x04),
        ] {
            t.get_len(2).map_err(|e| e.to_string())?;
            t.set_cur(2, &frame::encode(0x01, pair, seq, flags, vec![]))
                .map_err(|e| format!("{label}: {e}"))?;
            seq += 1;
            std::thread::sleep(std::time::Duration::from_millis(120));
            match t.get_cur(2, 60) {
                Ok(r) => match frame::decode(&r) {
                    Some(fr) => {
                        let n = (r[12] as usize).min(12);
                        let pay: Vec<String> =
                            fr.payload[..n].iter().map(|b| format!("{b:02x}")).collect();
                        println!(
                            "r{round} {label}: type={:02x} echo={} crc={} len={} pay={}",
                            fr.frame_type,
                            fr.cmd == pair,
                            fr.crc_ok,
                            r[12],
                            pay.join(" ")
                        );
                    }
                    None => println!("r{round} {label}: NOT-AA"),
                },
                Err(e) => println!("r{round} {label}: GET_CUR ERR {e}"),
            }
        }
    }
    Ok(())
}

#[cfg(not(all(target_os = "linux", feature = "uvc")))]
fn main() {
    println!("probe_boot needs linux + uvc feature");
}
