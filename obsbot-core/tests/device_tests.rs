//! Device-engine tests against a mock XU transport: wire sequence, reply
//! matching on the echoed cmd pair, retries, and typed reply parsing.
//! Mock replies are built in the exact byte form captured from the device
//! (`aa 29 seq 00 0c 00 ck ck 04 0a c0 c1 pLen 00 t0 t1 <pLen bytes>` —
//! payload length at byte 12, payload DATA at byte 16, live replay 2026-11-07).

use obsbot_core::cmd;
use obsbot_core::device::{ObsbotUsb, Result, TransportError, XuTransport};
use obsbot_core::frame;
use std::collections::VecDeque;

#[derive(Default)]
struct MockXu {
    log: Vec<String>,
    sets: Vec<Vec<u8>>,        // every SET_CUR payload, in order
    replies: VecDeque<Vec<u8>>, // GET_CUR queue
    events: VecDeque<Vec<u8>>,  // EP6 queue
    set_count: usize,
    reply_for: Option<fn(&[u8]) -> Vec<u8>>, // synthesize reply from SET bytes
}

impl XuTransport for MockXu {
    fn get_len(&mut self, selector: u8) -> Result<u16> {
        self.log.push(format!("get_len {selector}"));
        Ok(60)
    }
    fn set_cur(&mut self, selector: u8, data: &[u8]) -> Result<()> {
        self.log.push(format!("set_cur {selector} len{}", data.len()));
        self.sets.push(data.to_vec());
        self.set_count += 1;
        if let Some(f) = self.reply_for {
            self.replies.push_back(f(data));
        }
        Ok(())
    }
    fn get_cur(&mut self, selector: u8, len: usize) -> Result<Vec<u8>> {
        self.log.push(format!("get_cur {selector} {len}"));
        self.replies
            .pop_front()
            .ok_or_else(|| TransportError::Io("reply queue empty".into()))
    }
    fn poll_event(&mut self) -> Option<Vec<u8>> {
        self.events.pop_front()
    }
}

/// Echo a SET as the device's `0x29` reply: cmd pair echoed at 10..11,
/// tag bytes swapped, `data` placed at byte 16 with its length at byte 12.
fn reply_with(set: &[u8], data: &[u8]) -> Vec<u8> {
    let mut r = vec![0u8; 60];
    r[0] = 0xAA;
    r[1] = frame::TYPE_REPLY;
    r[2] = set[2];
    r[4] = 0x0C;
    r[8] = 0x04;
    r[9] = 0x0A;
    r[10] = set[10];
    r[11] = set[11];
    r[12] = data.len() as u8;
    r[16..16 + data.len().min(44)].copy_from_slice(&data[..data.len().min(44)]);
    let ck = frame::frame_crc(&r);
    r[6..8].copy_from_slice(&ck);
    r
}

fn reply_empty(set: &[u8]) -> Vec<u8> {
    reply_with(set, &[])
}

/// Same but rewrite the echoed pair so it never matches.
fn wrong_pair(set: &[u8]) -> Vec<u8> {
    let mut r = reply_empty(set);
    r[10] = 0xFF;
    r[11] = 0xFF;
    let ck = frame::frame_crc(&r);
    r[6..8].copy_from_slice(&ck);
    r
}

#[test]
fn request_follows_captured_sequence() {
    let mut m = MockXu::default();
    m.reply_for = Some(reply_empty);
    let mut dev = ObsbotUsb::new(m);
    let f = dev.request(&cmd::gimbal_attitude()).unwrap();
    assert_eq!(f.cmd, [0x08, 0x18]);
    assert!(f.crc_ok);
    assert!(f.is_reply());
    let log = dev.transport_mut().log.clone();
    assert_eq!(
        log,
        vec!["get_len 2", "set_cur 2 len60", "get_cur 2 60"],
        "must match the captured GET_LEN → SET_CUR → GET_CUR cadence"
    );
}

#[test]
fn mismatched_reply_retries_then_errors() {
    let mut m = MockXu::default();
    m.reply_for = Some(wrong_pair);
    let mut dev = ObsbotUsb::new(m);
    let r = dev.request(&cmd::gimbal_stop());
    assert!(matches!(r, Err(TransportError::Protocol(_))));
    assert_eq!(dev.transport_mut().set_count, 4, "4 attempts");
}

#[test]
fn event_frame_satisfies_matching_without_new_set() {
    let mut m = MockXu::default();
    // captured preset-name reply #1, byte-exact (type 0x29, "Preset1")
    let mut ev = frame::encode(0x01, [0x04, 0x3B], 7, 0x0A, vec![]);
    ev[1] = frame::TYPE_REPLY;
    ev[8] = 0x04;
    ev[9] = 0x0A;
    ev[12] = 7;
    ev[16..23].copy_from_slice(b"Preset1");
    let ck = frame::frame_crc(&ev);
    ev[6..8].copy_from_slice(&ck);
    m.events.push_back(ev);
    let mut dev = ObsbotUsb::new(m);
    let f = dev.request(&cmd::preset_name(0)).unwrap();
    assert_eq!(f.cmd, [0x04, 0x3B]);
    assert_eq!(f.payload, b"Preset1");
    assert_eq!(dev.transport_mut().set_count, 0, "no SET was needed");
}

#[test]
fn preset_request_is_type_21_indexed() {
    let mut m = MockXu::default();
    m.reply_for = Some(reply_empty);
    let mut dev = ObsbotUsb::new(m);
    dev.request(&cmd::preset_name(1)).unwrap();
    let s = &dev.transport_mut().sets[0];
    assert_eq!(s[1], 0x21, "frame type byte");
    assert_eq!(&s[10..12], &[0x04, 0x3B], "cmd pair");
    assert_eq!(s[12], 4, "arg length byte");
    assert_eq!(s[16], 1, "slot index at byte 16");
    // captured slot-0 sub-tag shape preserved
    dev.request(&cmd::preset_name(0)).unwrap();
    assert_eq!(&dev.transport_mut().sets[1][12..18], &[4, 0, 0xBE, 0x07, 0, 0]);
}

#[test]
fn preset_names_reads_three_slots() {
    let mut m = MockXu::default();
    m.reply_for = Some(|set: &[u8]| {
        let name: &[u8] = match set[16] {
            0 => b"Preset1",
            1 => b"P2",
            2 => b"P3",
            _ => b"",
        };
        reply_with(set, name)
    });
    let mut dev = ObsbotUsb::new(m);
    assert_eq!(dev.preset_names(3).unwrap(), vec!["Preset1", "P2", "P3"]);
}

#[test]
fn attitude_parses_euler_f32_from_payload() {
    let mut m = MockXu::default();
    m.reply_for = Some(|set: &[u8]| {
        let mut d = Vec::with_capacity(24);
        for v in [1.5f32, -2.25, 90.0] {
            d.extend_from_slice(&v.to_le_bytes());
        }
        d.resize(24, 0);
        reply_with(set, &d)
    });
    let mut dev = ObsbotUsb::new(m);
    assert_eq!(dev.attitude().unwrap(), [1.5, -2.25, 90.0]);
}

#[test]
fn serial_reads_ascii_payload() {
    let mut m = MockXu::default();
    m.reply_for = Some(|set: &[u8]| reply_with(set, b"RMOWLHGB081JIL"));
    let mut dev = ObsbotUsb::new(m);
    assert_eq!(dev.serial().unwrap(), "RMOWLHGB081JIL");
}

#[test]
fn command_pulls_reply_to_keep_device_queue_clean() {
    let mut m = MockXu::default();
    m.reply_for = Some(reply_empty);
    let mut dev = ObsbotUsb::new(m);
    dev.gimbal_speed(0.5, -0.25, 0.0).unwrap();
    let log = dev.transport_mut().log.clone();
    assert_eq!(log, vec!["get_len 2", "set_cur 2 len60", "get_cur 2 60"]);
}

#[test]
fn seq_increments_per_request() {
    let mut m = MockXu::default();
    m.reply_for = Some(reply_empty);
    let mut dev = ObsbotUsb::new(m);
    dev.request(&cmd::gimbal_attitude()).unwrap();
    dev.request(&cmd::gimbal_attitude()).unwrap();
    let sets = &dev.transport_mut().sets;
    assert_eq!(sets.len(), 2);
    assert_eq!(sets[0][2], 0, "first SET seq");
    assert_eq!(sets[1][2], 1, "second SET seq");
}

#[test]
fn raw_pan_tilt_uses_selector_4() {
    let mut m = MockXu::default();
    let mut dev = ObsbotUsb::new(m);
    dev.raw_pan_tilt(&[0x01, 0x02, 0x03]).unwrap();
    let log = dev.transport_mut().log.clone();
    assert_eq!(log, vec!["get_len 4", "set_cur 4 len3"]);
}
