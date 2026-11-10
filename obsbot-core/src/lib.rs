//! obsbot-core — reverse-engineered OBSBOT camera control protocol.
//!
//! Source of truth: decompile of `libdev.dll` (OBSBOT Center, Windows),
//! SHA-256 2486d1f3fe8be947fe62f6392a1f24641607f5d3b0d865cd31b1eef18d254bd8.
//! Docs: `../obsbot-center/reports/OBSBOT_camera_control_isolation.md`
//! and `analysis/cmd_payload_layouts.md`.
//!
//! Protocol summary (Tiny 2 Lite, capture-verified 2026-11-07 — no HID):
//! - Vendor frames ride the UVC Extension Unit (GUID {9a1e7291-…}, unit 2,
//!   VC interface 0) via GET_LEN/SET_CUR/GET_CUR class controls on EP0;
//!   async events push on interrupt-IN EP 0x86.
//! - Frame (60-byte padded): `AA | type | seq | 00 | 0C 00 | crc LE |
//!   tag | flags | cmd0 cmd1 | args…`; replies echo the cmd pair; see
//!   `frame` for the split-CRC and byte-exact layout.
//! - Zoom/pan/tilt video functions are standard UVC (use V4L2 on Linux) —
//!   this crate covers the vendor surface: gimbal, presets, tracking,
//!   power, status.

pub mod cmd;
pub mod device;
pub mod frame;
#[cfg(all(target_os = "linux", feature = "uvc"))]
pub mod transport;
#[cfg(target_os = "linux")]
pub mod linux_kernel;
#[cfg(all(target_os = "linux", feature = "stream"))]
pub mod stream;

pub use device::{ObsbotUsb, TransportError, XuTransport};

/// OBSBOT device USB identity (Tiny 2 Lite).
pub const VID: u16 = 0x3564;
pub const PID: u16 = 0xFEF9;
