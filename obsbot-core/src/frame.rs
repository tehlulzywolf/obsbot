//! Byte-exact vendor wire codec for the OBSBOT Tiny 2 Lite.
//!
//! Provenance (all from the 2026-11-07 Windows USBPcap capture of OBSBOT
//! Center driving a live Tiny 2 Lite (`VID_3564 PID_FEF9`), then replayed
//! byte-for-byte from WSL against the physical camera — 11/11 SETs accepted,
//! device answered every one):
//!
//! * The Tiny 2 Lite has NO HID interface. Frames ride the UVC Extension
//!   Unit {9a1e7291-6843-4683-6d92-39bc7906ee49} (VideoControl iface 0,
//!   bUnitID 2) as 60-byte SET_CUR/GET_CUR payloads; replies arrive either
//!   as the GET_CUR data stage or as async reports on EP 0x86.
//! * Frame layout (60-byte transfer, zero-padded; bytes past the payload
//!   are leftover app-buffer memory the device ignores):
//!
//!   | off | meaning                                             |
//!   |-----|-----------------------------------------------------|
//!   |  0  | magic `0xAA`                                        |
//!   |  1  | frame type: `0x01` set-cmd, `0x21`/`0x25` variants, |
//!   |     | `0x29` reply, `0x89` async EP6 event                |
//!   |  2  | sequence number (increments per session)            |
//!   |  3  | `0x00`                                            |
//!   |  4  | `0x0C` constant                                   |
//!   |  5  | `0x00`                                            |
//!   | 6,7 | checksum, LE (see `frame_crc`)                      |
//!   |  8  | request `0x0A` / reply `0x0D`                      |
//!   |  9  | flags (observed `0x02`, `0x0D`)                     |
//!   | 10  | command pair byte 0 (`cmd0`)                        |
//!   | 11  | command pair byte 1 (`cmd1`)                        |
//!   |12.. | arguments, command-specific                         |
//!
//! * Commands are *direct two-byte wire pairs* at bytes 10..11 — captured
//!   pairs: `08 18` gimbal attitude (repeated x5 at boot), `08 04`, `42 C4`
//!   gimbal speed, reply pair `04 3B`. The old `(set<<12 | id)` remap word
//!   from static DLL analysis was an internal function-dispatch key, NOT a
//!   wire encoding (captured pairs are far too small).
//! * A SET_CUR is answered by an `0x29` reply on the SAME control pipe;
//!   GET_CUR then pulls the pending reply. `cmd` matching keys off the
//!   bytes-10/11 pair (the reply echoes it).
//! * The `libdev.dll` checksum (`FUN_1800ff1a0`) is this exact table walk
//!   (pinned by 26/27 captured frames — the 27th is a truncated EP6 report,
//!   see analysis/caps/ckdef.py): running state `(b4, b6)` seeded `FF FF`,
//!   per byte `idx = b6 ^ x; b6 = b4 ^ A[idx]; b4 = B[idx]`, span = header
//!   bytes 0..12 with the checksum slots (6,7) treated as zero, final
//!   `v = !((b4 << 8) | b6)`, stored little-endian (byte6 = v.lo).
//!   Table provenance: dumped from the DLL's FUN_1800ff1a0 code (one-byte
//!   XOR constants) and cross-validated against all captured frames.

/// SET_CUR/GET_CUR transfer length declared by GET_LEN (60).
pub const XFER_LEN: usize = 60;
pub const MAGIC: u8 = 0xAA;
pub const TYPE_SET: u8 = 0x01;
pub const TYPE_SET_VARIANT: u8 = 0x21;
pub const TYPE_SET_VARIANT2: u8 = 0x25;
pub const TYPE_REPLY: u8 = 0x29;
pub const TYPE_EVENT: u8 = 0x89;
/// Fixed header byte 4.
pub const CONST_C: u8 = 0x0C;
/// Request tag (byte 8).
pub const TAG_REQUEST: u8 = 0x0A;
/// Reply tag (byte 8).
pub const TAG_REPLY: u8 = 0x0D;
/// Fixed bytes 3, 5.
pub const ZERO_AT_3: usize = 3;
pub const ZERO_AT_5: usize = 5;
/// Bytes 0..12 = checksummed header; args start at 12.
pub const HEADER_LEN: usize = 12;
pub const ARGS_OFF: usize = 12;

pub const TYPE_MASK: u8 = 0x3F; // type byte & 0x3f = category

pub const CRC_SPLIT_A: [u8; 256] = [
    0x00, 0xc1, 0x81, 0x40, 0x01, 0xc0, 0x80, 0x41, 0x01, 0xc0, 0x80, 0x41, 0x00, 0xc1, 0x81, 0x40,
    0x01, 0xc0, 0x80, 0x41, 0x00, 0xc1, 0x81, 0x40, 0x00, 0xc1, 0x81, 0x40, 0x01, 0xc0, 0x80, 0x41,
    0x01, 0xc0, 0x80, 0x41, 0x00, 0xc1, 0x81, 0x40, 0x00, 0xc1, 0x81, 0x40, 0x01, 0xc0, 0x80, 0x41,
    0x00, 0xc1, 0x81, 0x40, 0x01, 0xc0, 0x80, 0x41, 0x01, 0xc0, 0x80, 0x41, 0x00, 0xc1, 0x81, 0x40,
    0x01, 0xc0, 0x80, 0x41, 0x00, 0xc1, 0x81, 0x40, 0x00, 0xc1, 0x81, 0x40, 0x01, 0xc0, 0x80, 0x41,
    0x00, 0xc1, 0x81, 0x40, 0x01, 0xc0, 0x80, 0x41, 0x01, 0xc0, 0x80, 0x41, 0x00, 0xc1, 0x81, 0x40,
    0x00, 0xc1, 0x81, 0x40, 0x01, 0xc0, 0x80, 0x41, 0x01, 0xc0, 0x80, 0x41, 0x00, 0xc1, 0x81, 0x40,
    0x01, 0xc0, 0x80, 0x41, 0x00, 0xc1, 0x81, 0x40, 0x00, 0xc1, 0x81, 0x40, 0x01, 0xc0, 0x80, 0x41,
    0x01, 0xc0, 0x80, 0x41, 0x00, 0xc1, 0x81, 0x40, 0x00, 0xc1, 0x81, 0x40, 0x01, 0xc0, 0x80, 0x41,
    0x00, 0xc1, 0x81, 0x40, 0x01, 0xc0, 0x80, 0x41, 0x01, 0xc0, 0x80, 0x41, 0x00, 0xc1, 0x81, 0x40,
    0x00, 0xc1, 0x81, 0x40, 0x01, 0xc0, 0x80, 0x41, 0x01, 0xc0, 0x80, 0x41, 0x00, 0xc1, 0x81, 0x40,
    0x01, 0xc0, 0x80, 0x41, 0x00, 0xc1, 0x81, 0x40, 0x00, 0xc1, 0x81, 0x40, 0x01, 0xc0, 0x80, 0x41,
    0x00, 0xc1, 0x81, 0x40, 0x01, 0xc0, 0x80, 0x41, 0x01, 0xc0, 0x80, 0x41, 0x00, 0xc1, 0x81, 0x40,
    0x01, 0xc0, 0x80, 0x41, 0x00, 0xc1, 0x81, 0x40, 0x00, 0xc1, 0x81, 0x40, 0x01, 0xc0, 0x80, 0x41,
    0x01, 0xc0, 0x80, 0x41, 0x00, 0xc1, 0x81, 0x40, 0x00, 0xc1, 0x81, 0x40, 0x01, 0xc0, 0x80, 0x41,
    0x00, 0xc1, 0x81, 0x40, 0x01, 0xc0, 0x80, 0x41, 0x01, 0xc0, 0x80, 0x41, 0x00, 0xc1, 0x81, 0x40,
];

pub const CRC_SPLIT_B: [u8; 256] = [
    0x00, 0xc0, 0xc1, 0x01, 0xc3, 0x03, 0x02, 0xc2, 0xc6, 0x06, 0x07, 0xc7, 0x05, 0xc5, 0xc4, 0x04,
    0xcc, 0x0c, 0x0d, 0xcd, 0x0f, 0xcf, 0xce, 0x0e, 0x0a, 0xca, 0xcb, 0x0b, 0xc9, 0x09, 0x08, 0xc8,
    0xd8, 0x18, 0x19, 0xd9, 0x1b, 0xdb, 0xda, 0x1a, 0x1e, 0xde, 0xdf, 0x1f, 0xdd, 0x1d, 0x1c, 0xdc,
    0x14, 0xd4, 0xd5, 0x15, 0xd7, 0x17, 0x16, 0xd6, 0xd2, 0x12, 0x13, 0xd3, 0x11, 0xd1, 0xd0, 0x10,
    0xf0, 0x30, 0x31, 0xf1, 0x33, 0xf3, 0xf2, 0x32, 0x36, 0xf6, 0xf7, 0x37, 0xf5, 0x35, 0x34, 0xf4,
    0x3c, 0xfc, 0xfd, 0x3d, 0xff, 0x3f, 0x3e, 0xfe, 0xfa, 0x3a, 0x3b, 0xfb, 0x39, 0xf9, 0xf8, 0x38,
    0x28, 0xe8, 0xe9, 0x29, 0xeb, 0x2b, 0x2a, 0xea, 0xee, 0x2e, 0x2f, 0xef, 0x2d, 0xed, 0xec, 0x2c,
    0xe4, 0x24, 0x25, 0xe5, 0x27, 0xe7, 0xe6, 0x26, 0x22, 0xe2, 0xe3, 0x23, 0xe1, 0x21, 0x20, 0xe0,
    0xa0, 0x60, 0x61, 0xa1, 0x63, 0xa3, 0xa2, 0x62, 0x66, 0xa6, 0xa7, 0x67, 0xa5, 0x65, 0x64, 0xa4,
    0x6c, 0xac, 0xad, 0x6d, 0xaf, 0x6f, 0x6e, 0xae, 0xaa, 0x6a, 0x6b, 0xab, 0x69, 0xa9, 0xa8, 0x68,
    0x78, 0xb8, 0xb9, 0x79, 0xbb, 0x7b, 0x7a, 0xba, 0xbe, 0x7e, 0x7f, 0xbf, 0x7d, 0xbd, 0xbc, 0x7c,
    0xb4, 0x74, 0x75, 0xb5, 0x77, 0xb7, 0xb6, 0x76, 0x72, 0xb2, 0xb3, 0x73, 0xb1, 0x71, 0x70, 0xb0,
    0x50, 0x90, 0x91, 0x51, 0x93, 0x53, 0x52, 0x92, 0x96, 0x56, 0x57, 0x97, 0x55, 0x95, 0x94, 0x54,
    0x9c, 0x5c, 0x5d, 0x9d, 0x5f, 0x9f, 0x9e, 0x5e, 0x5a, 0x9a, 0x9b, 0x5b, 0x99, 0x59, 0x58, 0x98,
    0x88, 0x48, 0x49, 0x89, 0x4b, 0x8b, 0x8a, 0x4a, 0x4e, 0x8e, 0x8f, 0x4f, 0x8d, 0x4d, 0x4c, 0x8c,
    0x44, 0x84, 0x85, 0x45, 0x87, 0x47, 0x46, 0x86, 0x82, 0x42, 0x43, 0x83, 0x41, 0x81, 0x80, 0x40,
];

/// Checksum over the 12-byte header (bytes 6,7 treated as zero, since the
/// checksum itself lives there). Returns `[byte6, byte7]` (LE of `!state`).
pub fn frame_crc(f: &[u8]) -> [u8; 2] {
    let (mut b4, mut b6) = (0xFFu8, 0xFFu8);
    for (i, &x0) in f[..HEADER_LEN].iter().enumerate() {
        let x = if i == 6 || i == 7 { 0 } else { x0 };
        let idx = (b6 ^ x) as usize;
        b6 = b4 ^ CRC_SPLIT_A[idx];
        b4 = CRC_SPLIT_B[idx];
    }
    let v = !(((b4 as u16) << 8) | b6 as u16);
    [v as u8, (v >> 8) as u8]
}

/// Sub-checksum for indexed query requests (frame types `0x21`/`0x25`),
/// placed at request bytes 14..15. Firmware rule pinned 2026-11-07 by
/// exhaustive 65536-seed brute-force over captured requests; the seed
/// below is unique and independently verified on 4 frames — preset
/// reads idx 0/1/2 (`be 07`/`bf fb`/`bf bf`) and the `cd 10` type-0x25
/// query (payload `55 53 00 cc` → `5f 8f`). The device rejects preset
/// reads whose sub-checksum is wrong (probe: `sub=00 00` → no reply),
/// but type-`0x01` requests carry `00 00` and are accepted.
pub fn sub_ck(payload: &[u8]) -> [u8; 2] {
    let (mut b4, mut b6) = (0x14u8, 0x01u8);
    for &x in payload {
        let idx = (b6 ^ x) as usize;
        b6 = b4 ^ CRC_SPLIT_A[idx];
        b4 = CRC_SPLIT_B[idx];
    }
    let v = !(((b4 as u16) << 8) | b6 as u16);
    [v as u8, (v >> 8) as u8]
}

/// A decoded vendor frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub frame_type: u8,
    pub seq: u8,
    pub crc_ok: bool,
    pub tag: u8,
    pub flags: u8,
    /// Command pair (bytes 10..11): requests send it, replies echo it.
    pub cmd: [u8; 2],
    /// Bytes 14..15. On `0x29` replies this is a per-command constant the
    /// firmware ships inside its static reply template (NOT a checksum —
    /// live replay 2026-11-07 returned byte-identical replies to the
    /// original capture, same sub-tag, across sessions). On preset-index
    /// requests (`04 3B` type 0x21) the app sends varying tags here
    /// (`be 07`, `bf fb`, `bf bf`) that the device does not validate.
    pub sub_tag: [u8; 2],
    /// Requests: args at byte 12. Replies: `payload_len` bytes at byte 16
    /// (capture+live verified: `aa 29 seq 00 0c 00 ck ck 04 0a c0 c1
    /// pLen 00 t0 t1 <pLen bytes>`; trailing bytes past 16+pLen are stale
    /// firmware buffer contents and must be ignored).
    pub payload: Vec<u8>,
    /// Reply payload length byte (frame byte 12); 0 on requests.
    pub payload_len: u8,
}

impl Frame {
    pub fn is_reply(&self) -> bool {
        self.frame_type & TYPE_MASK == TYPE_REPLY & TYPE_MASK
    }
    pub fn is_event(&self) -> bool {
        self.frame_type == TYPE_EVENT
    }
}

/// Build a 60-byte wire buffer for a request frame (type pinned by the
/// capture: plain commands `0x01`, preset reads `0x21`, `cd 10` query `0x25`).
pub fn encode(
    frame_type: u8,
    cmd_pair: [u8; 2],
    seq: u8,
    flags: u8,
    args: impl Into<Vec<u8>>,
) -> Vec<u8> {
    let args = args.into();
    let mut f = vec![0u8; XFER_LEN];
    f[0] = MAGIC;
    f[1] = frame_type;
    f[2] = seq;
    f[4] = CONST_C;
    f[8] = TAG_REQUEST;
    f[9] = flags;
    f[10] = cmd_pair[0];
    f[11] = cmd_pair[1];
    f[ARGS_OFF..ARGS_OFF + args.len().min(XFER_LEN - ARGS_OFF)]
        .copy_from_slice(&args[..args.len().min(XFER_LEN - ARGS_OFF)]);
    let ck = frame_crc(&f);
    f[6] = ck[0];
    f[7] = ck[1];
    f
}

/// Decode any `0xAA` buffer >= 12 bytes (GET_CUR replies, EP6 reports).
pub fn decode(buf: &[u8]) -> Option<Frame> {
    let f = if buf.len() > XFER_LEN { &buf[..XFER_LEN] } else { buf };
    if f.len() < HEADER_LEN || f[0] != MAGIC {
        return None;
    }
    let ck = frame_crc(f);
    let crc_ok = ck == [f[6], f[7]];
    let end = f.len().min(XFER_LEN);
    let is_rep = f[1] & TYPE_MASK == TYPE_REPLY & TYPE_MASK;
    let (payload, payload_len) = if is_rep && f.len() >= 16 {
        let n = (16 + f[12] as usize).min(f.len());
        (f[16..n].to_vec(), f[12])
    } else {
        let mut p = f[ARGS_OFF.min(end)..end].to_vec();
        while p.last() == Some(&0) {
            p.pop();
        }
        (p, 0)
    };
    Some(Frame {
        frame_type: f[1],
        seq: f[2],
        crc_ok,
        tag: f[8],
        flags: f[9],
        cmd: [f[10], f[11]],
        sub_tag: if f.len() >= 16 { [f[14], f[15]] } else { [0, 0] },
        payload,
        payload_len,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hx(s: &str) -> Vec<u8> {
        s.split_whitespace().map(|b| u8::from_str_radix(b, 16).unwrap()).collect()
    }

    #[test]
    fn crc_matches_captured_request() {
        // first captured SET_CUR frame, header bytes (cmd pair 08 18)
        let f = hx("aa 01 00 00 0c 00 91 5c 0a 0d 08 18");
        assert_eq!(frame_crc(&f), [0x91, 0x5C]);
    }

    #[test]
    fn crc_matches_live_reply() {
        // live-replay GET_CUR pull, header (cmd pair 04 3b echoed)
        let f = hx("aa 29 0b 00 0c 00 96 e9 04 0a 04 3b");
        assert_eq!(frame_crc(&f), [0x96, 0xE9]);
    }

    #[test]
    fn encode_matches_capture() {
        // gimbal attitude request: aa 01 00 .. cmd 08 18, all-zero args
        let wire = encode(0x01, [0x08, 0x18], 0, 0x0D, vec![]);
        assert_eq!(hx("aa 01 00 00 0c 00 91 5c 0a 0d 08 18"), &wire[..12]);
        assert!(wire[12..].iter().all(|&b| b == 0));
        assert_eq!(wire.len(), XFER_LEN);
    }

    #[test]
    fn preset_read_request_matches_capture() {
        // captured preset-name read #1: type 0x21, flags 0x04, len=4, idx=0
        let wire = encode(
            0x21,
            [0x04, 0x3B],
            0x0B,
            0x04,
            vec![0x04, 0x00, 0xBE, 0x07, 0x00, 0x00],
        );
        assert_eq!(hx("aa 21 0b 00 0c 00 df a2 0a 04 04 3b 04 00 be 07 00 00"), &wire[..18]);
    }

    #[test]
    fn decode_reply_uses_len_at_12_data_at_16() {
        // live/capture golden: preset-name reply #1 ("Preset1", 7 bytes).
        // Everything past byte 16+7 is stale firmware buffer — ignored.
        let mut f = hx("aa 29 0b 00 0c 00 96 e9 04 0a 04 3b 07 00 19 ff \
                        50 72 65 73 65 74 31 14 00 00 00 00 00 00 00 00 00 00 00 \
                        37 be 8b bc bd 72 00 00 37 be 8b bc bd 72 01 00 00 00 00 00 00 00 00 00");
        f.resize(XFER_LEN, 0);
        let fr = decode(&f).unwrap();
        assert!(fr.is_reply());
        assert!(fr.crc_ok); // header CRC over 12-byte span
        assert_eq!(fr.cmd, [0x04, 0x3B]);
        assert_eq!(fr.payload_len, 7);
        assert_eq!(fr.sub_tag, [0x19, 0xFF]);
        assert_eq!(fr.payload, b"Preset1");
    }

    #[test]
    fn decode_reply_zero_payload_keeps_full_length() {
        // attitude reply (camera level): 24-byte payload of three f32 zeros
        let mut f = hx("aa 29 00 00 0c 00 a0 49 0d 0a 08 18 18 00 59 fc \
                        00 00 00 00 00 00 00 00 00 00 00 00 00 00");
        f.resize(XFER_LEN, 0);
        let fr = decode(&f).unwrap();
        assert_eq!(fr.payload_len, 0x18);
        assert_eq!(fr.payload.len(), 0x18);
        assert!(fr.payload.iter().take(12).all(|&b| b == 0));
    }

    #[test]
    fn decode_roundtrip_and_echo() {
        let wire = encode(0x01, [0x42, 0xC4], 7, 0x02, vec![0xF4, 0x01, 0xF4, 0x01]);
        let fr = decode(&wire).unwrap();
        assert!(fr.crc_ok);
        assert_eq!(fr.cmd, [0x42, 0xC4]);
        assert_eq!(fr.seq, 7);
        assert_eq!(fr.payload, vec![0xF4, 0x01, 0xF4, 0x01]);
        // corrupted header byte -> crc_ok false
        let mut bad = wire.clone();
        bad[4] ^= 0x01;
        assert!(!decode(&bad).unwrap().crc_ok);
        // event frame decodes with magic stripped (EP6 raw starts at aa)
        assert!(decode(&hx("aa 89 00 00 0c 00 00 00 0d 00 04 3b 01")).is_some());
    }

    #[test]
    fn sub_ck_matches_captured_query_checksums() {
        // the four independently captured type-21/25 requests
        assert_eq!(sub_ck(&[0, 0, 0, 0]), [0xBE, 0x07]); // preset idx 0
        assert_eq!(sub_ck(&[1, 0, 0, 0]), [0xBF, 0xFB]); // preset idx 1
        assert_eq!(sub_ck(&[2, 0, 0, 0]), [0xBF, 0xBF]); // preset idx 2
        assert_eq!(sub_ck(&[0x55, 0x53, 0x00, 0xCC]), [0x5F, 0x8F]); // cd 10 query
    }

    #[test]
    fn rejects_short_and_bad_magic() {
        assert!(decode(&[0xAA; 11]).is_none());
        let mut f = encode(0x01, [8, 0x18], 0, 0, vec![]);
        f[0] = 0x55;
        assert!(decode(&f).is_none());
    }
}
