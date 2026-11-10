//! Raw reply dump: replays the byte-exact frames captured 2026-11-07 from
//! OBSBOT Center (tx9.json SET_CUR data stages) through the live XU
//! transport and prints every GET_CUR reply in full hex, plus an ASCII
//! view — used to pin reply payload layouts in `cmd`/`device`.
//!
//!   cargo run --features uvc --example rawdump
//!
//! Frames are 12-byte header + args from the capture, zero-padded to 60
//! (captured tail bytes were app-buffer leftovers the device ignores;
//! the checksum only covers bytes 0..12).

#[cfg(all(target_os = "linux", feature = "uvc"))]
fn main() -> Result<(), String> {
    use obsbot_core::device::XuTransport;
    use obsbot_core::frame;
    use obsbot_core::transport::NusbXu;

    // (label, 60-byte frame hex) — verbatim captured SET_CUR frames
    let frames: [(&str, &str); 14] = [
        ("attitude-0818      ", "aa0100000c00915c0a0d08180000000000000000000000000000"),
        ("gimbal-0804        ", "aa0101000c00c1500a0d080400000000000000000000000000000"),
        ("gimbal-4819        ", "aa0102000c00c0960a0d481900000000000000000000000000000"),
        ("gimbal-c818        ", "aa0103000c0031530a0dc81800000000000000000000000000000"),
        ("speed-42c4         ", "aa0104000c00d6730a0242c400000000700000000000000000000"),
        ("basic-0401         ", "aa0105000c0095840a04040100000000000000000000000000000"),
        ("basic-0468         ", "aa0106000c00a5a50a04046800000000700000000000000000000"),
        ("gimbal-443b        ", "aa0107000c00859d0a04443b5c020000000000000000000000000"),
        ("v25-cd10           ", "aa2508000c00cd160a12cd1004005f8f555300000000000000000"),
        ("c229               ", "aa0109000c00e6040a02c22900000000000000000000000000000"),
        ("4229               ", "aa010a000c0077cb0a024229004c0000000000000000000000000"),
        ("preset-043b-1      ", "aa210b000c00dfa20a04043b0400be07000000000000000000000"),
        ("preset-043b-2      ", "aa210c000c006e780a04043b0400bffb010000000000000000000"),
        ("preset-043b-3      ", "aa210d000c003fbd0a04043b0400bfbf020000000000000000000"),
    ];

    let xu = NusbXu::open_camera()?;
    let mut cam = obsbot_core::ObsbotUsb::new(xu);
    let t = cam.transport_mut();
    t.get_len(2).map_err(|e| e.to_string())?;

    let ascii = |b: &u8| {
        if b.is_ascii_graphic() || *b == b' ' {
            *b as char
        } else {
            '.'
        }
    };

    for (label, hexs) in frames {
        let mut f: Vec<u8> = hexs
            .as_bytes()
            .chunks(2)
            .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap())
            .collect();
        f.resize(60, 0);
        match t.set_cur(2, &f) {
            Ok(()) => {}
            Err(e) => {
                println!("{label} SET STALL {e:?}");
                continue;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(120));
        match t.get_cur(2, 60) {
            Ok(r) => {
                let hx: String = r.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ");
                let asc: String = r.iter().map(ascii).collect();
                let dec = frame::decode(&r);
                let summary = match &dec {
                    Some(d) => format!(
                        "type={:02x} seq={} cmd={:02x}{:02x} crc_ok={}",
                        d.frame_type, d.seq, d.cmd[0], d.cmd[1], d.crc_ok
                    ),
                    None => "NOT-AA".into(),
                };
                println!("{label} REPLY {summary}\n  hex: {hx}\n  asc: {asc}");
            }
            Err(e) => println!("{label} GET_CUR ERR {e:?}"),
        }
        // drain any EP6 event
        while let Some(ev) = t.poll_event() {
            let hx: String = ev.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ");
            println!("{label} EP6: {hx}");
        }
    }
    Ok(())
}

#[cfg(not(all(target_os = "linux", feature = "uvc")))]
fn main() {
    println!("rawdump is Linux + `uvc` feature (camera attached via usbipd).");
}
