# OBSBOT Center — Camera-Control Core Isolation (for Rust/Linux port)

Read this as a dated lab notebook. Sections 1–6 are the 2026-11-06 first pass.
They describe a HID write path and a frame with no checksum. Both are wrong for
the Tiny 2 Lite: section 7b and `analysis/HANDOFF.md` replace them. The camera
has no HID interface. Vendor commands go out on the UVC extension unit, and the
frames carry a CRC. The panel in `obsbot-core` is the current program. Section 8
below is the historical next-step list from that week, not the work left today.

Target: OBSBOT Center 2.0.15.186, analyzed 2026-11-06.
Binary of interest: `C:\Program Files\OBSBOT Center\bin\libdev.dll`
SHA-256: 2486d1f3fe8be947fe62f6392a1f24641607f5d3b0d865cd31b1eef18d254bd8 (2,246,280 bytes)
Ghidra 12.1.4 decompilation; PDB referenced (libdev.pdb GUID 2ca512b4-4286-3796-eac9-e73cb000b3c2) but not shipped; RTTI present → Ghidra recovered `Device::*`, `DeviceCenter::*` member names.

## 1. What to port — and what to ignore

| Component | Role | Port? |
|---|---|---|
| **libdev.dll** (2.2 MB) | THE camera-control library: device enum/open, command protocol, gimbal/zoom/presets/params/OTA. 613 decorated `SYMDLL_` exports = a complete C API. | **YES — this is the core** |
| OBSBOT_Main.exe (92 MB) | Media pipeline + device threads host; feeds libdev callbacks | Partially (only its threading/queue logic, reimplemented) |
| OBSBOT_Center.exe (14.7 MB) | Qt6 GUI / orchestration, ZeroMQ IPC | No (rewrite GUI natively or CLI) |
| rm.dll (libobs fork), FFmpeg, Tencent liteav, XMagic/FaceTrackNoir | Streaming, virtual camera, AI tracking engine | No (Linux has v4l2loopback + PipeWire instead; tracking engine only if you want auto-tracking parity) |
| Qt6*, libcurl, libzmq | Framework glue | No |
| rm-kcp.dll | KCP UDP transport for phone-app remote control | Only if wireless/phone control wanted |

The Windows-only pieces you must replace on Linux:
- `HidD_*` HID API → hidapi / hidraw / nusb (direct equivalents)
- `SetupDi*` + KSCATEGORY_VIDEO device paths → udev / /sys/class/hidraw
- DirectShow/KS "uvc device" path (only used for one USB branch; the HID report path is what the logs show in active use)

## 2. Verified architecture (from decompilation)

Call chain: 613 exported `SYMDLL_*` C functions → `DeviceCenter::sharedInstance()` → `Device::` member (RTTI-named, e.g. `Device::aiGetGimbalPresetNameWithIdR@0x18001d908`) → **sync request engine `FUN_180007490`** → transport (virtual, reached via vtable) → **`FUN_180178220` = HID write choke point** (`HidD_SetOutputReport`; exactly 2 call sites in the whole DLL) → camera. RX side: `HidD_GetInputReport` read-thread → frame parse (`ParsePlain` / `ParseNet` strings) → seq-matched to pending list → caller woken (condvar) or async listener callback.

### Sync engine semantics (FUN_180007490, decompiled)
- Internal request struct (u16-indexed): `[0]=frame len`, `+2 (u32)=msg type (0=request,1,2 = req/ack/rsp)`, `[4]=seq`, `[5],[6]=capability/product index bytes`, `[7]=cmd_id`, `[8]=cmd_set`, `[10]=protocol mode (3 = encrypted network/OTA variant)`, payload from byte 28.
- `seq`: 16-bit atomic counter per device (`LOCK(); *p = *p+1; UNLOCK()`), written into header.
- Pending list (mutex-protected linked list @dev+0x478) keyed by seq; response removes entry; timeout 1 s, one resend ("try to resend packet: seq = %d, flag = %d"), then error `-3`/`-2`.
- USB device mode flag at dev+0x3c; when USB-UVC mode: max payload 0x3C = **60 bytes** ("data len %d > uvc max payload len 60").
- HID report write (FUN_180178220): buffer = `[byte0 = subcode][payload]`, length = HID `PreparsedData` OutputReportByteLength (device reports 64), plus a second zero-subcode write of remaining bytes.
- Log lines confirm protocol fields: `cmd=59; sub cmd=%d; isRequest=1, isLast=1, devMode=0` (gimbal preset get = cmd_id 59 = 0x3B, cmd_set 0), `cmd=1024; status=0x0` telemetry.

### Wire frame (FUN_180100450 "Plain" encoder, decompiled — the key artifact)
```
offset  size  field
0       1     magic 0xAA
1       2     total frame length, 12 bits (LE u16 & 0xFFF)
3       1     flags: bit0-1 = msg type (0->b1, 1->b00, 2->b01), bit4-5 = 0b01 constant
4       2     seq (LE)
6       3     zero
9       1     (hi<<4|lo): nibble lookups of request bytes [6] and [5]
              in runtime-registered std::map tables (product/protocol-version code)
10      2     wire cmd: 12 bits = map(cmd_set -> map(cmd_id -> wirecode)) XOR-folded,
              top 4 bits = map(cmd_set -> setcode)
12      n     payload (copied from internal struct byte 28)
```
No CRC in the plain frame — reliability = seq/ack + resend at engine level.
Encrypted/network mode (cmd_set 3 / protocol 3) uses FUN_1801006c0 + token string (OTA/auth; relevant only for firmware upgrade and phone remote).

### cmd_id → wire-code tables — DUMPED (2026-11-06, second pass)
All table content lives in MSVC global static initializers (functions at 0x1800015b0–0x1800019b0, each followed by `atexit(cleanup)`), with pairs packed as u64 immediates `(key:u32 | value:u32<<32)`. Fully decoded; machine-readable copy: `analysis/cmd_tables.json`.

**Key structural finding:** these maps are *remap/override* tables, not the command namespace.
- Outer map `DAT_180218ef8` keys: **{1,2,3,5} only** (remote/app/BLE/KCP protocol sets). **cmd_set 0 (local USB control) is absent** → header wire-cmd word falls back to the `0xFFF` sentinel and the **true cmd_id (u16) travels as payload bytes 0–1** (verified in `Device::aiGetGimbalPresetNameWithIdR`: payload starts `3B 00 03 00` for cmd_id 0x3B=59). For a USB-only Rust port the header is fixed-pattern: `wire word = 0x0FFF` (+ setcode 0) and the payload carries the command.
- `DAT_180218f28` (byte-9 nibble map, applied to internal[5] and internal[6] = version indices): near-identity {0..6→same, 7→8, 8→0xB, 9→0xC, 0xA→0xD, 0xB→0xE, 0xC→0xF}.
- `DAT_180218e28` (cmd_set → setcode for wire-word bits 12–15): {0:0, 1:1, 2:2, 3:3, 4:4, 5:8, 6:0xD}.
- Inner maps (app-cmd → wirecode overrides), entries:
  - set 1 `DAT_180218e58`: {0:0x1CD, 1:0x3C4, 2:0x3C2, 3:0x3D2, 4:0x1D2}
  - set 2 `DAT_180218f08`: {0:1, 2:4, 3:6, 4:0xA, 5:0x22, 6:0x23}
  - set 3 `DAT_180218ed8`: {0:0x23, 7:0x35, 8:0x36, 0xA:0x3A, 0xD:0x51, 0xE:0x66, 0xF:0x67, 0x10:0x90, 0x11:0x91, 0x13:0x92, 0x15:0x94, 0x5B:0x93, 0x65:0xEE}
  - set 5 `DAT_180218ee8`: {0:1, 3:0x10, 4:0x60, 5:0x61, 6:0x64, 7:0x65, 8:0x63, 9:0x66, 0xA:0x20, 0xB:0x69, 0xC:0x6A, 0xD:0x6B}
The per-cmd **payload layouts** (arg structs per cmd_id) are the remaining unknown for the full feature set — best recovered by walking the ~613 exported `SYMDLL_*` wrappers (each builds its payload inline, same pattern as the gimbal-preset sample), or confirmed live via one USB sniff session (app logs contain no raw frame bytes; `raw_hex=` hits are only ASCII preset names).

## 3. Device facts (from app logs, %APPDATA%\OBSBOT_Center\centerLogs)
- USB: `VID_3564&PID_FEF9` (OBSBOT Tiny 2 Lite), MI_00–MI_03 composite + `HID_*` interface, KSCATEGORY_VIDEO GUID path.
- Same device reachable over BLE (`OBSBOT Tiny2 Lite-BLE-FEF90061D114`) and wireless-KCP dongle — libdev supports dev_mode 0=USB, plus wifi/ble modes.
- Control traffic observed in logs is the **USB-HID path** (gimbal preset polling, 1 Hz telemetry).

## 4. Export surface (porting API map)
613 exports, MFC-decorated but plain C linkage (`SYMDLL_` prefix, `__stdcall`). Categories (counts from earlier pefile classification): device mgmt/connect/open/enum, gimbal/PTZ (~50), zoom, presets, focus/exposure/WB/camera params, tracking/gesture (results arrive as callbacks from the AI engine; libdev only forwards), LED/light, audio/mic/speaker/volume, OTA/firmware, callbacks/listeners, info/version. Reimplementing this C ABI in Rust (`#[no_mangle] extern "system"`) gives a drop-in `libdev.so`.

## 5. Suggested Rust crate layout
```
obsbot-core/
  src/transport/{hid.rs (nusb|hidapi), kcp.rs, ble.rs}   // vtable == enum Transport
  src/proto/{frame.rs (0xAA codec), engine.rs (seq/ack/resend state machine)}
  src/cmds/{gimbal.rs, zoom.rs, params.rs, preset.rs, ota.rs}   // cmd tables
  src/telemetry.rs   // cmd=1024 status reports -> events
  src/api.rs         // C ABI mirror of SYMDLL_* you actually need
```
Start minimal: open HID (VID 3564), send `get gimbal preset` (cmd_id 59) exactly as decompiled, verify ack, then implement PTZ move/stop + zoom + telemetry. Everything else is more of the same pattern.

## 6. FINAL DECOMPILE — DONE (2026-11-06)

All ~30 core control exports (`Device::cameraSet*`, `gimbal*`, `ai*`, presets,
zoom, power/sleep, boot status) fully decompiled in Ghidra and distilled into
`analysis/cmd_payload_layouts.md`. Key outcomes:

1. **One universal request struct** for every vendor command:
   `[kind u16][pad][chan u8 @+8][0x0B00 u16 @+9][cmd_word u32 @+0C = (set<<16)|cmd_id][sn_word][product_type][args...]`.
   Wire payload = struct bytes from +0x0C. Rust needs one `build_request()` + per-command arg packers.
2. **Command inventory recovered** (set 3 = AI/gimbal, set 2 = raw gimbal, set 1 = camera basic):
   pan/tilt speed 0x2_0002, euler 0x3_0004, stop 0x3_000C, state 0x3_000B, tracking enable
   0x3_000D/0x3_0062, track mode 0x3_0011, track types 0x3_0071/0x72, presets
   0x3_003E/0x37/0x35/0x36/0x5B, attitude 0x2_0000, rst pos 0x2_0001/0x06,
   zoom stop 0x1_0034, auto-sleep 0x1_0091, run status 0x1_0002, boot status 0x1_0017 …
3. **Zoom + pan/tilt are standard UVC** (CameraControl_Pan/Tilt/Zoom via
   KSPROPERTY/UVC-ext), not vendor HID → on Linux they come free via V4L2
   `uvcvideo`; only gimbal/preset/tracking/power commands need hand-rolled frames.
4. `wakeUpDevice`/`connectDev`/`getDevList` carry no wire protocol (C++
   lifecycle plumbing) → replace with plain hidraw enumeration in Rust.

## 7. Effort estimate (updated after final decompile)
- Frame codec + HID transport + first working vendor commands (stop, speed,
  state, presets): **2–4 days** (struct + cmd inventory fully decoded).
- Full USB feature set incl. UVC PTZ/zoom via V4L2: **1–2 weeks**.
- Remaining unknown: byte-for-byte header nibble/len confirmation — one
  10-minute USBPcap/usbmon sniff of any live control (e.g. preset goto) closes it.

## 7b. CORRECTION pass 2026-11-07 (live WSL + targeted re-decompile)
- **Tiny 2 Lite has NO HID interface** (live `lsusb -v`, analysis/tiny2lite_desc.txt).
  The HID path (FUN_180178220) is for HID SKUs only. Lite rides the **UVC
  Extension Unit** GUID `{9a1e7291-6843-4683-6d92-39bc7906ee49}` (U-path), opened
  on Windows via KSPROPERTY on a KSCATEGORY_VIDEO KsDevice (app logs confirm
  `dev_mode=0`, KSCATEGORY_VIDEO path, cmd=59 presets answered in ~80 ms).
- XU mapping (FUN_1800087f0): KSPROPERTY Id == XU selector; **selector 2 =
  generic 0xAA-framed pipe** (FUN_1800083c0 sends every vendor frame there,
  payload always padded to 60 bytes), selector 6 = cameraSetAiModeU raw write.
- **TX frames DO carry a checksum** (supersedes the "no CRC" note in section 2):
  FUN_1800ffce0 post-encodes to BIG-endian length/seq, byte-swaps bytes 10/11,
  computes dual-table CRC-16 (init 0xFFFF, final NOT) over the whole frame and
  stores it BE at bytes 6-7. `src/frame.rs` + tests updated accordingly (green).
- WSL2 (Ubuntu 26.04, kernel 6.18) + usbipd-win works: camera attached as real
  USB, `uvcvideo` binds. Raw nusb XU probe sends were accepted but no replies
  yet — pending one USBPcap sniff of the real app to pin exact setup/poll timing.
  USBPcap 1.5.4 installed via winget; class filter activates after one reboot.

## 8. Historical next actions (2026-11-06, done or superseded)

1. USBPcap of Center's extension-unit traffic, then a working UVC transport.
   The transport is `UVCIOC_CTRL_QUERY` in `obsbot-core`. A later capture was
   not required once the command bytes were recovered from `OBSBOT_Main.exe`.
2. The crate exists at `obsbot-core/`. The Linux path is the UVC extension
   unit, not hidraw. The Tiny 2 Lite has no HID interface (section 7b).
3. A drop-in `libdev.so` was not built. The panel calls the Rust API directly.

What is still open is the unverified list in `analysis/HANDOFF.md`.
