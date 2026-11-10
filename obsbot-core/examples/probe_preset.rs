//! Preset-request variants probe: does the device care about frame type
//! (0x21 vs 0x01), flags, or the 2 "sub-tag" bytes before the index?
//! cargo run --features uvc --example probe_preset

#[cfg(all(target_os = "linux", feature = "uvc"))]
fn main() -> Result<(), String> {
    use obsbot_core::device::XuTransport;
    use obsbot_core::frame;
    use obsbot_core::transport::NusbXu;

    let xu = NusbXu::open_camera()?;
    let mut cam = obsbot_core::ObsbotUsb::new(xu);
    let t = cam.transport_mut();
    t.get_len(2).map_err(|e| e.to_string())?;

    let mut seq = 0x30u8;
    let mk = |idx: u8| {
        let ck = frame::sub_ck(&[idx, 0, 0, 0]);
        vec![0x04, 0x00, ck[0], ck[1], idx, 0]
    };
    let bad = frame::sub_ck(&[2, 0, 0, 0]); // idx2's checksum, wrong for idx1
    let variants: [(&str, u8, u8, Vec<u8>); 4] = [
        // control: captured-equivalent, computed checksum for idx0
        ("idx0 computed-ck (ctrl)", 0x21, 0x04, mk(0)),
        // deliberate corruption: idx1 payload with idx2's checksum
        ("idx1 wrong-ck (expect silence)", 0x21, 0x04, {
            let mut a = mk(1);
            a[2] = bad[0]; // flip low byte of the checksum
            a
        }),
        // generalization test: NEVER-captured index with computed checksum
        ("idx3 computed-ck (generalize)", 0x21, 0x04, mk(3)),
        // and idx 200 — beyond any plausible preset count
        ("idx200 computed-ck", 0x21, 0x04, mk(200)),
    ];
    for (label, ft, fl, args) in variants {
        let wire = frame::encode(ft, [0x04, 0x3B], seq, fl, args);
        seq += 1;
        t.get_len(2).map_err(|e| e.to_string())?;
        t.set_cur(2, &wire).map_err(|e| format!("{label}: {e}"))?;
        std::thread::sleep(std::time::Duration::from_millis(120));
        match t.get_cur(2, 60) {
            Ok(r) => {
                let d = frame::decode(&r);
                match d {
                    Some(fr) => {
                        let name: String = fr
                            .payload
                            .iter()
                            .map(|&b| if b.is_ascii_graphic() { b as char } else { '.' })
                            .collect();
                        println!("{label}: type={:02x} cmd={:02x}{:02x} crc={} len={} pay={}",
                            fr.frame_type, fr.cmd[0], fr.cmd[1], fr.crc_ok, r[12], name);
                    }
                    None => println!("{label}: NOT-AA {}", r.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")),
                }
            }
            Err(e) => println!("{label}: GET_CUR ERR {e}"),
        }
    }
    Ok(())
}

#[cfg(not(all(target_os = "linux", feature = "uvc")))]
fn main() {
    println!("probe_preset is Linux + `uvc` feature.");
}
