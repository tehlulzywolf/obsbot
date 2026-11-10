//! Codec tests grounded in the 2026-11-07 USBPcap capture (`cap1.pcap` /
//! `tx9.json`) and the live WSL replay — golden frames are byte-exact
//! device traffic, not reconstructions.

use obsbot_core::cmd;
use obsbot_core::frame;

fn hx(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect()
}

/// All 14 SET_CUR frames captured 2026-11-07 (first 18 bytes; the
/// remainder of each 60-byte transfer is zero/leftover pad).
const CAPTURED_SETS: &[&str] = &[
    "aa 01 00 00 0c 00 91 5c 0a 0d 08 18 00 00 00 00 00 00",
    "aa 01 01 00 0c 00 c1 50 0a 0d 08 04 00 00 00 00 00 00",
    "aa 01 02 00 0c 00 c0 96 0a 0d 48 19 00 00 00 00 00 00",
    "aa 01 03 00 0c 00 31 53 0a 0d c8 18 00 00 00 00 00 00",
    "aa 01 04 00 0c 00 d6 73 0a 02 42 c4 00 00 00 00 70 00",
    "aa 01 05 00 0c 00 95 84 0a 04 04 01 00 00 00 00 00 00",
    "aa 01 06 00 0c 00 a5 a5 0a 04 04 68 00 00 00 00 70 00",
    "aa 01 07 00 0c 00 85 9d 0a 04 44 3b 5c 02 00 00 00 00",
    "aa 25 08 00 0c 00 cd 16 0a 12 cd 10 04 00 5f 8f 55 53",
    "aa 01 09 00 0c 00 e6 04 0a 02 c2 29 00 00 00 00 00 00",
    "aa 01 0a 00 0c 00 77 cb 0a 02 42 29 00 4c 00 00 00 00",
    "aa 21 0b 00 0c 00 df a2 0a 04 04 3b 04 00 be 07 00 00",
    "aa 21 0c 00 0c 00 6e 78 0a 04 04 3b 04 00 bf fb 01 00",
    "aa 21 0d 00 0c 00 3f bd 0a 04 04 3b 04 00 bf bf 02 00",
];

#[test]
fn every_captured_frame_passes_crc() {
    for s in CAPTURED_SETS {
        let f = hx(s);
        let d = frame::decode(&f).unwrap_or_else(|| panic!("undecodable: {s}"));
        assert!(d.crc_ok, "crc mismatch for {s}");
        assert!(!d.is_reply());
    }
}

#[test]
fn golden_request_decodes() {
    // boot gimbal-attitude poll (cmd pair 08 18), first captured SET
    let f = hx("aa 01 00 00 0c 00 91 5c 0a 0d 08 18");
    let d = frame::decode(&f).unwrap();
    assert!(d.crc_ok);
    assert_eq!(d.cmd, [0x08, 0x18]);
    assert_eq!(d.seq, 0);
    assert_eq!(d.frame_type, frame::TYPE_SET);
    assert!(!d.is_reply());
}

#[test]
fn golden_reply_decodes() {
    // live-replay GET_CUR pull (cmd pair 04 3b echoed, tag bytes swapped)
    let f = hx("aa 29 0b 00 0c 00 96 e9 04 0a 04 3b");
    let d = frame::decode(&f).unwrap();
    assert!(d.crc_ok);
    assert_eq!(d.cmd, [0x04, 0x3B]);
    assert!(d.is_reply());
}

#[test]
fn encode_reproduces_captured_attitude_frame() {
    let f = cmd::gimbal_attitude();
    let wire = frame::encode(0x01, f.cmd_pair(), 0, f.flags, f.to_payload());
    let want = hx(CAPTURED_SETS[0]);
    assert_eq!(&wire[..18], &want[..18]);
    assert_eq!(wire.len(), frame::XFER_LEN);
}

#[test]
fn encode_reproduces_captured_preset_reads() {
    // type-0x21 indexed preset-name reads, byte-exact for all three slots
    for (slot, idx) in (11u8..14).zip(0u8..3) {
        let p = cmd::preset_name(idx);
        let wire = frame::encode(
            p.ftype.unwrap_or(0x01),
            p.cmd_pair(),
            slot, // captured seqs are 0x0B, 0x0C, 0x0D
            p.flags,
            p.to_payload(),
        );
        let want = hx(CAPTURED_SETS[slot as usize]);
        assert_eq!(&wire[..18], &want[..18], "preset slot {idx}");
    }
}

#[test]
fn encode_matches_captured_speed_frame() {
    // capture: flags 0x02, pair 42 c4, i16LE*100 args, third slot 0x0070
    let g = cmd::gimbal_speed(0.0, 0.0, 1.12);
    let wire = frame::encode(0x01, g.cmd_pair(), 4, g.flags, g.to_payload());
    let want = hx(CAPTURED_SETS[4]);
    assert_eq!(&wire[..18], &want[..18]);
}

#[test]
fn tracking_pairs_match_the_decompile() {
    let human = cmd::tracking_type(1);
    assert_eq!(human.cmd_pair(), [0x71, 0x30]);
    assert_eq!(human.to_payload(), &[0, 0, 0, 0, 0, 0, 0, 0]);
    let group = cmd::tracking_type(0x1E);
    assert_eq!(group.cmd_pair(), [0x72, 0x30]);
    assert_eq!(group.to_payload(), &[0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(cmd::tracking_clear(1).cmd_pair(), [0x70, 0x30]);
    assert_eq!(cmd::tracking_clear(0x1E).cmd_pair(), [0x73, 0x30]);
    assert_eq!(cmd::tracking_clear(1).to_payload(), &[] as &[u8]);
    assert_eq!(cmd::tracking_enable(true).cmd_pair(), [0x0D, 0x30]);
    assert_eq!(cmd::tracking_enable(true).to_payload(), &[1]);
    assert_eq!(cmd::tracking_mode(0).cmd_pair(), [0x11, 0x30]);
}

#[test]
fn header_corruption_detected_payload_blind_ok() {
    let f = hx(CAPTURED_SETS[4]);
    let mut bad = f.clone();
    bad[2] ^= 0x40; // flip a seq bit (inside the checksummed span)
    assert!(!frame::decode(&bad).unwrap().crc_ok);
    let mut ok = f.clone();
    ok[16] ^= 0xFF; // payload byte (0x70 arg slot): outside the 12-byte span by design
    assert!(frame::decode(&ok).unwrap().crc_ok);
}

#[test]
fn rejects_bad_input() {
    assert!(frame::decode(&[0xAA; 11]).is_none());
    let mut f = frame::encode(0x01, [8, 0x18], 0, 0, vec![]);
    f[0] = 0;
    assert!(frame::decode(&f).is_none());
}
