//! boot_status timing forensics: for pair 17 10, do SET_CUR then TWO
//! GET_CUR pulls (immediate + delayed) and drain EP86 events between,
//! dumping full reply heads to locate where the 2-byte payload appears.

#[cfg(all(target_os = "linux", feature = "uvc"))]
fn main() -> Result<(), String> {
    use obsbot_core::device::XuTransport;
    use obsbot_core::frame;
    use obsbot_core::transport::NusbXu;

    let xu = NusbXu::open_camera()?;
    let mut cam = obsbot_core::ObsbotUsb::new(xu);
    let t = cam.transport_mut();
    t.get_len(2).map_err(|e| e.to_string())?;

    let mut seq = 0x60u8;
    let pair = [0x17u8, 0x10];
    for round in 0..5 {
        t.get_len(2).map_err(|e| e.to_string())?;
        t.set_cur(2, &frame::encode(0x01, pair, seq, 0x0d, vec![]))
            .map_err(|e| e.to_string())?;
        seq += 1;
        for (pull, delay_ms) in [(0u32, 0u32), (1, 150), (2, 300)] {
            if delay_ms > 0 {
                std::thread::sleep(std::time::Duration::from_millis(delay_ms as u64));
            }
            // drain events first
            while t.poll_event().is_some() {}
            match t.get_cur(2, 60) {
                Ok(r) => println!(
                    "r{round} pull{pull}: {}",
                    r.iter()
                        .take(20)
                        .map(|b| format!("{b:02x}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                ),
                Err(e) => println!("r{round} pull{pull}: ERR {e}"),
            }
        }
    }
    Ok(())
}

#[cfg(not(all(target_os = "linux", feature = "uvc")))]
fn main() {
    println!("probe_boot2 needs linux + uvc feature");
}
