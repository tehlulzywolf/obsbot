// repro3: field-catalog sweep over selector 6 — one SET_CUR per AiMode field,
// stream ACTIVE the whole time (panel conditions). Neither a second write nor
// streaming kills the link (repro/repro2), so the suspect is field-specific
// firmware code paths: e.g. mic/AGC fields that re-init the audio subsystem.
// First field that kills the link = last line printed + dmesg "connection closed".
// Panel stopped, supervisor paused. Run as root in WSL:
//   cargo run --features codec --example repro3_fields
use obsbot_core::device::ObsbotUsb;
use obsbot_core::linux_kernel::{Capture, V4lXu};
use std::thread::sleep;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?;
    println!("MARK repro3 start epoch={}", t.as_secs());
    let xu = V4lXu::open_camera()?;
    let mut d = ObsbotUsb::new(xu);
    println!("serial: {}", d.serial()?);
    let mut cap = Capture::start_mjpeg_fps(1280, 720, 60)?;
    for _ in 0..10 {
        let _ = cap.next_frame();
    }
    println!("stream warm");

    // Ordered: protective field first, benign/mode fields middle,
    // audio + AI-reconfig fields last (highest kill suspicion).
    let fields: &[(&str, u16, &[u8])] = &[
        ("0112 disable-sleep-without-stream", 0x0112, &[1]),
        ("0113 mic-during-sleep off", 0x0113, &[0]),
        ("0100 MediaMode default", 0x0100, &[0]),
        ("0101 HDR off", 0x0101, &[0]),
        ("0103 FaceAE off", 0x0103, &[0]),
        ("0105 BgMode off", 0x0105, &[0]),
        ("0110 BgColor 0", 0x0110, &[0]),
        ("0111 BgEnabled (inverted)", 0x0111, &[1]),
        ("0118 LED ctrl 0", 0x0118, &[0]),
        ("011B MicDistance 0", 0x011B, &[0]),
        ("1021 FieldView 1x4", 0x1021, &[1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0]),
        ("0222 AudioMode 0", 0x0222, &[0, 0]),
        ("0215 AudioCtrlState 0", 0x0215, &[0, 0]),
        ("0117 AGC on", 0x0117, &[1]),
        ("010A NoiseReduce off", 0x010A, &[0]),
        ("011C MicEnabled on", 0x011C, &[1]),
        ("0520 Gesture off", 0x0520, &[0, 0, 0, 0, 0, 0]),
        ("0216 AiWorkMode=0 Track=1", 0x0216, &[0, 1]),
    ];
    for (name, id, val) in fields {
        let r = d.ai_field(*id, val);
        let ok = r.is_ok();
        println!("FIELD {name}: write-> {:?}", r.map(|_| "OK"));
        if !ok {
            println!("!! write failed — link state unknown, stopping sweep");
            break;
        }
        sleep(Duration::from_millis(200));
        let frame_ok = cap.next_frame().is_ok();
        let aa = d.serial().is_ok();
        println!("   stream_frame={frame_ok} aa_serial_ok={aa}");
        if !aa {
            println!("!! AA path dead after this field — this field is the killer");
            break;
        }
    }
    println!("END reached");
    Ok(())
}
