//! xuprobe: map the OBSBOT Tiny 2 Lite vendor channel on Linux.
//!
//! The Tiny 2 Lite has NO USB HID interface (verified live): vendor frames
//! ride on UVC Extension Unit #2 (GUID {9a1e7291-...}) via class control
//! transfers on the VideoControl interface, replies on interrupt endpoint
//! 0x86. Frames are now the CORRECTED wire format (BE length/seq, CRC-16
//! at bytes 6..7, wire-cmd word) from `obsbot_core::frame`.
//!
//! Probe plan:
//!   1. GET_CUR selector scan (two wValue encodings),
//!   2. EP6 idle watch,
//!   3. correctly-encoded GET frames (attitude/boot/preset-list) via
//!      SET_CUR over selectors 1..=8 x both encodings, decoding EP6 replies.
use nusb::{
    transfer::{ControlIn, ControlOut, ControlType, In, Interrupt, Recipient},
    MaybeFuture,
};
use obsbot_core::{cmd, frame};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const VID: u16 = 0x3564;
const PID: u16 = 0xFEF9;
const XU_UNIT: u16 = 2; // Extension Unit bUnitID
const CT_IFACE: u16 = 0; // VideoControl interface number
const EP_IN: u8 = 0x86; // VC interrupt endpoint (16B, interval 8)
const T: Duration = Duration::from_millis(800);

fn hex(d: &[u8]) -> String {
    d.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
}

fn wvalue(enc: u8, sel: u8) -> u16 {
    // E1: selector high, CS_XU(0x02) low. E2: CS_XU high, selector low.
    if enc == 1 {
        ((sel as u16) << 8) | 0x02
    } else {
        (0x02 << 8) | sel as u16
    }
}

fn get_cur(dev: &nusb::Device, enc: u8, sel: u8) -> Result<Vec<u8>, nusb::transfer::TransferError> {
    dev.control_in(
        ControlIn {
            control_type: ControlType::Class,
            recipient: Recipient::Interface,
            request: 0x81, // GET_CUR (capture: a1 81)
            value: wvalue(enc, sel),
            index: (XU_UNIT << 8) | CT_IFACE,
            length: 60,
        },
        T,
    )
    .wait()
}

fn set_cur(dev: &nusb::Device, enc: u8, sel: u8, data: &[u8]) -> Result<(), nusb::transfer::TransferError> {
    dev.control_out(
        ControlOut {
            control_type: ControlType::Class,
            recipient: Recipient::Interface,
            request: 0x01, // SET_CUR
            value: wvalue(enc, sel),
            index: (XU_UNIT << 8) | CT_IFACE,
            data,
        },
        T,
    )
    .wait()
}

/// Encode a capture-format AA frame (60-byte padded) for a request.
fn frame_of(req: &cmd::Req, seq: u8) -> Vec<u8> {
    frame::encode(0x01, req.cmd_pair(), seq, req.flags, req.to_payload())
}

/// Print an EP6/CUR buffer plus decoded frame view if it parses.
fn show(tag: &str, d: &[u8]) {
    let dec = frame::decode(d);
    match dec {
        Some(f) => println!(
            "EP6 {tag}: {}  -> AA type={:02X} seq={} cmd={:02X}{:02X} flags={:02X} crc_ok={} args={}",
            hex(d),
            f.frame_type,
            f.seq,
            f.cmd[0],
            f.cmd[1],
            f.flags,
            f.crc_ok,
            hex(&f.payload)
        ),
        None => println!("EP6 {tag}: {}", hex(d)),
    }
}

fn drain(rx: &mpsc::Receiver<Vec<u8>>, ms: u64) -> usize {
    let mut n = 0;
    let t0 = std::time::Instant::now();
    while t0.elapsed() < Duration::from_millis(ms) {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(d) => {
                show("RX", &d);
                n += 1;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }
    }
    n
}

fn main() {
    let info = nusb::list_devices()
        .wait()
        .unwrap()
        .into_iter()
        .find(|d| d.vendor_id() == VID && d.product_id() == PID)
        .expect("OBSBOT 3564:fef9 not found (attach via usbipd first)");
    let dev = info.open().wait().expect("open failed");
    println!("opened OBSBOT (bus {})", info.busnum());

    // Claim VC interface, kicking uvcvideo off temporarily.
    let iface = dev
        .detach_and_claim_interface(CT_IFACE as u8)
        .wait()
        .expect("claim interface 0 failed");
    println!("claimed VC interface (uvcvideo detached; it re-attaches on exit)");

    // EP6 interrupt-IN reader thread.
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let stop = Arc::new(AtomicBool::new(false));
    {
        let stop = stop.clone();
        thread::spawn(move || {
            let ep = match iface.endpoint::<Interrupt, In>(EP_IN) {
                Ok(ep) => ep,
                Err(e) => {
                    eprintln!("EP6 open: {e:?}");
                    return;
                }
            };
            use std::io::Read as _;
            let mut rd = ep.reader(64);
            let mut buf = [0u8; 64];
            loop {
                match rd.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("EP6 reader end: {e:?}");
                        break;
                    }
                }
                if stop.load(Ordering::Relaxed) {
                    break;
                }
            }
        });
    }

    // 0) Compliance strictness: bogus unit / bogus bRequest should stall
    //    on real firmware. If these "succeed", accepts prove nothing.
    println!("-- strictness checks --");
    for (unit, req, sel) in [
        (2u8, 0x01u8, 2u8),  // baseline real XU GET_CUR
        (9, 0x01, 2),        // nonexistent XU unit
        (2, 0x03, 2),        // GET_MIN on real unit
        (2, 0x01, 99),       // GET_CUR bogus selector
    ] {
        let r = dev.control_in(ControlIn {
            control_type: ControlType::Class,
            recipient: Recipient::Interface,
            request: req,
            value: ((sel as u16) << 8) | 0x02,
            index: ((unit as u16) << 8) | CT_IFACE,
            length: 60,
        }, T).wait();
        match r {
            Ok(d) => println!("unit{unit} req{req:02X} sel{sel}: OK {}", hex(&d[..12])),
            Err(e) => println!("unit{unit} req{req:02X} sel{sel}: ERR {e:?}"),
        }
    }

    // 1) GET_CUR selector scan, both encodings.
    for enc in [1u8, 2] {
        println!("-- GET_CUR scan enc{enc} --");
        for sel in 1..=19u8 {
            match get_cur(&dev, enc, sel) {
                Ok(d) => {
                    let nonz = d.iter().any(|b| *b != 0);
                    println!(
                        "sel {sel:02}: {}{}",
                        hex(&d[..d.len().min(24)]),
                        if nonz { " *NONZERO*" } else { "" }
                    );
                }
                Err(e) => println!("sel {sel:02}: ERR {e:?}"),
            }
        }
    }

    // 2) EP6 idle traffic for 2s.
    println!("-- EP6 idle watch 2s --");
    drain(&rx, 2000);

    // 3) Ground truth from libdev.dll: generic frames ride XU selector 2,
    //    SET_CUR always carries a FULL 60-byte (0x3c) zero-padded payload,
    //    and replies are polled back via GET_CUR (async KS set->get), EP6
    //    as backup.
    let att = cmd::gimbal_attitude();
    let fa = frame_of(&att, 0x20); // capture-verified format
    let fb = frame::encode(0x01, [0xFF, 0xFF], 0x21, 0x0D, &att.cmd_word().to_le_bytes());
    let pad = |d: &[u8]| {
        let mut v = d.to_vec();
        v.resize(60, 0);
        v
    };

    for (vname, f0) in [("CAPTURED", fa.as_slice()), ("SENTINEL", fb.as_slice())] {
        let fp = pad(f0);
        println!("-- padded {vname} (60B): {} --", hex(&fp[..18]));
        if set_cur(&dev, 1, 2, &fp).is_err() {
            println!("SET sel2 failed");
            continue;
        }
        // poll GET_CUR sel 2 for up to 3 s
        let t0 = std::time::Instant::now();
        let mut hit = false;
        while t0.elapsed() < Duration::from_millis(3000) && !hit {
            thread::sleep(Duration::from_millis(250));
            if let Ok(d) = get_cur(&dev, 1, 2) {
                if d.iter().any(|b| *b != 0) {
                    println!(">>> CUR HIT {vname} after {:?}:", t0.elapsed());
                    show("CUR", &d);
                    hit = true;
                }
            }
            while let Ok(d) = rx.try_recv() {
                show("EP6", &d);
                hit = true;
            }
        }
        if hit {
            break;
        }
        println!("-- {vname}: no reply via sel2, sweeping all selectors --");
        for sel in 1..=19u8 {
            if set_cur(&dev, 1, sel, &fp).is_ok() {
                if let Ok(d) = get_cur(&dev, 1, sel) {
                    if d.iter().any(|b| *b != 0) {
                        println!(">>> CUR HIT {vname} sel{sel}:");
                        show("CUR", &d);
                        hit = true;
                        break;
                    }
                }
                if drain(&rx, 150) > 0 {
                    println!(">>> EP6 HIT {vname} sel{sel}");
                    hit = true;
                    break;
                }
            }
        }
        if hit {
            break;
        }
    }

    stop.store(true, Ordering::Relaxed);
    std::process::exit(0);
}
