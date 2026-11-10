# Roadmap RE — Item 5 (Hand Tracking) & Item 14 (Track Speed / Offsets / Gestures)

Binary: `C:\Program Files\OBSBOT Center\bin\libdev.dll` (document `target-2486d1f3fe8b.dll`, image base 0x180000000).
Method: Ghidra decompile via REA of the export functions (addresses from `libdev_exports.csv`).

## Common command-struct protocol (re-confirmed in this pass)

All `_R`/offset exports build a stack struct passed to the sender `FUN_180008180` (async/fire-forget) or `FUN_180007490` (sendMsgSync, GET) or `FUN_180007aa0` (async-with-callback GET). Struct fields observed:

| offset | value | meaning (per established protocol) |
|---|---|---|
| +0x00 (u16) | 0x0C / 0x0D / 0x0E / 0x10 / 0x24 | kind/length word. 0x0C = GET / no-arg send (12B hdr), 0x0D = SET_SIMPLE no-serial with 1-byte arg, 0x0E = 2-byte arg, 0x10 = SET_ACK with 4-byte (float) arg, 0x24 = 36B (24B arg block). The sender treats `*param_2` as payload length (`data len %d > uvc max payload len %d` vs 0x3c), so the kind byte doubles as the length. |
| +0x08 (u32) | 1 | set by sender as `(serial_param & 0xff) ^ 1` — 1 when serial arg == 0 (no-serial path) |
| +0x0A (u8) | 3 | constant protocol/version byte in every export |
| +0x0C (u16) | 0x0b00 | constant (byte +0x0C = 0x0B) |
| +0x0E (u32) | 0xS_ID | command id with S nibble. Wire pair bytes10-11 = `[ID_low, S<<4]`. Every command in this report has S=3 → second pair byte `0x30`. |
| +0x14 (u32) | `dev+0x1668` | channel; == 3 selects the AA (advanced-attribute) serializer `FUN_1801006c0`. Tiny 2 Lite uses this AA path. |
| +0x18 (u32) | `dev+0x3c` | dev_mode. dev_mode==0 (Tiny 2 Lite): sender requires UVC device open (`dev+0x3e0 != 0`) and payload <= 0x3c. |
| +0x1C | args | u8 args at +0x1c, float at +0x1c, u32 LE args at +0x1c per size |

`FUN_180008180` (0x180008180) stamps a serial word from `dev+0x528` counter at struct+0x08-0x0a area and enqueues via `FUN_180012800`. No branch rewrites the kind on the dev_mode-0 path: **the kind/len byte and pair go on the wire exactly as built.**

All exports below return 0 on success, -1 on sender error; none have productType 0xB/0xC/0xD/0x10/0x12/0x13/0x16-only branches — all are valid on Tiny 2 Lite (product type 3, dev_mode 0).

---

## Item 5 — Hand tracking

### aiSetHandTrackModeR(AiHandTrackType) — 0x18001b570 (ordinal 99/100)
- Path: AA. kind/len `0x0D` (SET_SIMPLE no-serial). `+0x0E = 0x30049` → ID 0x49 (73), S=3 → **wire pair `49 30`**.
- Args: `+0x1c` = 1 byte, `(uchar)AiHandTrackType` param_1.
  Live result, 2026-11-09: byte 1 follows either hand and byte 0 turns tracking
  off. Left versus Right is not a separate argument. See `HANDOFF.md`.
- Enum names: **none in libdev.dll** — string search `(?i)hand|palm|victory|fist` yields only error strings ("error occurred in aiSetHandTrackModeR, ret_val = %d" @0x1801ce8f8) and export names; no "palm"/"stop"/"victory"/"fist"/"ok" strings anywhere. No switch/case inside the export (value is passthrough). Numeric domain 1 byte (0–255); typical OBSBOT hand-gesture set (e.g. palm/fist/stop/victory) is applied device-side. **UNVERIFIED-BY-EYES** which byte value enables tracking and which gestures are recognized.

### aiSetHandTrackGimbalEnabledR(bool) — 0x18001bad0 (ordinal 97)
- Path: AA. kind/len `0x0D`. `+0x0E = 0x30057` → ID 0x57 (87), S=3 → **pair `57 30`**.
- Args: `+0x1c` = 1 byte bool (0/1): whether hand tracking drives the gimbal.

### aiGetHandTrackStateR(AiHandTrackStateInfo*, int, callback, void*, GetMethod) — 0x18001ad90 (ordinal 34)
- Present in CSV (ordinal 34, rva 109968).
- Path: AA GET. kind/len `0x0C`, `+0x0E = 0x3004a` → ID 0x4A (74), S=3 → **pair `4a 30`**.
- Sync path (GetMethod != 1): `FUN_180007490` sendMsgSync; response buffer parsed into `AiHandTrackStateInfo` = **5 floats + 1 byte**: `[0..4]` floats (position/orientation; log line "Hand Track Init Position: %d %f %f %f %f %d" @0x1801ce7b8), `[5]` = byte at resp+0x14 cast to float (state/flag). Exact field semantics **UNVERIFIED-BY-EYES**.
- Special cases: `size param == -1` → struct switched to kind/len `0x0D` with arg `+0x1c = 0xff` (variant poll). `GetMethod == 1` → async dispatch via `FUN_180007aa0` with a std::function callback wrapper (LAB_18001ad60 thunk).

### Hand button: separate AA command, NOT a tracking_type value
- `aiSetTrackingModeR(AiVerticalTrackType)` @0x18001ac20 is the tracking-type setter and uses its **own** command `+0x0E = 0x30011` → pair `11 30`, kind/len 0x0D, 1-byte arg.
- Hand tracking lives in its own AA command family: mode `0x49`, state `0x4a`, init-pos set `0x4b`, reset `0x4d`, trigger `0x4e`, gimbal-enable `0x57`. The "Hand" button therefore maps to the hand-track command (or the gesture toggle, below), not to a value of the tracking-type enum. (Whether the Center UI "Hand" button specifically calls `aiSetHandTrackModeR` vs `aiSetGestureCtrlR` is host-side app logic — UNVERIFIED-BY-EYES from this DLL alone; libdev offers both as separate AA commands.)

### Hand-track init-pos cluster (context for item 5)
- `aiSetHandTrackInitPosR(PresetPosInfo*)` @0x18001afd0 — AA, kind/len `0x24` (36B: 12B hdr + 24B pos block), `+0x0E = 0x3004b` → **pair `4b 30`**. Pos block built by `FUN_180018d20(info, out, productType)`: for productType in bitmask `0x0200340C` (includes low ids, i.e. NOT Tiny 2 Lite id 3? — mask bit3 = 0x8 not set, so **Lite uses the scaled path**) writes `+0x00` = raw u32 info[0] (preset index), then 8× int16 = floats×100 (fields 1–4) and corner combos `x*100 - h*50`, `y*100 - h*50`, `h*50 + x*100`, `h*50 + y*100` from PresetPosInfo fields 0x17/0x18/0x19.
- `aiGetHandTrackInitPosR` @0x18001b0b0 (ordinal 33) — GET, same PresetPosInfo readback (log "Hand Track Init Position: %d %f %f %f %f %f").
- `aiRstHandTrackInitPosR(void)` @0x18001b450 — kind/len `0x0C`, `+0x0E = 0x3004d` → **pair `4d 30`**, no args.
- `aiTrgHandTrackInitPosR(void)` @0x18001b4e0 — kind/len `0x0C`, `+0x0E = 0x3004e` → **pair `4e 30`**, no args.

---

## Item 14 — Track speed, offsets, gestures

### aiSetTrackSpeedTypeR(AiTrackSpeedType) — 0x180021220 (ordinal 124)
- Present in CSV. Path: AA. kind/len `0x0D`. `+0x0E = 0x30068` → ID 0x68 (104), S=3 → **pair `68 30`**.
- Args: `+0x1c` = 1 byte enum, passthrough. No name strings anywhere (search `(?i)speed|slow|...` → only gimbal/zoom speed error strings). Enum values/names **UNVERIFIED-BY-EYES** (numeric domain 0–255; presumably smooth/fast presets applied on-device).

### aiSetHorizontalOffset(float) — 0x1800209e0 (ordinal 104)
- Path: AA. kind/len `0x10` (SET_ACK). `+0x0E = 0x30079` → ID 0x79 (121), S=3 → **pair `79 30`**.
- Args: `+0x1c` = float32 LE (offset value, radians/normalized per app; unit UNVERIFIED-BY-EYES).
- Getter `aiGetHorizontalOffset(float&)` @0x180020a80 — GET `0x0C`, `+0x0E = 0x30078` → pair `78 30`; response: float at resp buffer offset 0.

### aiSetVerticalOffset(float) — 0x180020b50 (ordinal 126)
- Path: AA. kind/len `0x10` (SET_ACK). `+0x0E = 0x3007b` → ID 0x7B (123), S=3 → **pair `7b 30`**.
- Args: `+0x1c` = float32 LE.
- Getter `aiGetVerticalOffset(float&)` @0x180020bf0 — GET, `+0x0E = 0x3007a` → pair `7a 30`.
- Related: `aiSetAutoOffset(bool)` @0x180020870 — AA `0x0D`, `+0x0E = 0x30076` → pair `76 30`, 1-byte bool (auto-offset toggle).

### cameraSetGestureControlU(bool, uchar, bool, ushort) — 0x1800f87a0 (ordinal 324)
- Path: **sel6** (U-suffixed → `FUN_1800087f0(dev, selector=6, buf)` AiMode selector-6 60B struct; no AA pair).
- Buffer layout (values from byte 2): `[0..1]` u16 field id = **0x0520 (1312)**; `[2]` = param_1 (bool enable); `[3]` = param_2 (uchar, sub/mode id); `[4]` = param_3 (bool); `[5..6]` = param_4 (u16 LE, value e.g. timeout/level). Remainder zero.
- Semantics of param_2/param_3/param_4 **UNVERIFIED-BY-EYES** (no strings; log only prints the low byte of the field header).

### aiGetGestureModeR — ABSENT
- Not in `libdev_exports.csv` and no procedure matching `(?i)gesture` is named GestureMode (full gesture procedure list enumerated: aiSetGestureCtrlR/IndividualR, aiSet/GetGestureParaR, aiSet/GetGestureTrackParaR, aiSetGestureTargetSelectR/ZoomR/DynamicZoomR/ZoomFactorR/Record, cameraSet/GetVirtualTrackGestureR, cameraSetGestureControlU). Gesture "mode" is host-side state or one of the `DevGestureParaType`/`DevGestureTrackParaType` enum params.
- Closest AA gesture master toggle: `aiSetGestureCtrlR(bool)` @0x180018f20 (ordinal 74) — AA, kind/len `0x0E`, `+0x0E = 0x3000d` → **pair `0d 30`**, args `+0x1c` = 5 (sub-command selector, matches the "selector 5" style) and `+0x1d` = 1-byte bool on/off.
- `cameraSetVirtualTrackGestureR(DevVirtualTrackGesture)` @0x1800fbf40 / `cameraGetVirtualTrackGestureR` @0x1800fbe50 — the virtual-gesture pair (not requested in detail; AA R-path, ordinals 404/254).

---

## dev_mode-0 (Tiny 2 Lite) wire summary

On Lite (dev+0x3c == 0, dev+0x1668 == 3, serial arg 0 → no-serial, kind 0x0D stays 0x0D): wire bytes10-11 carry `[ID, 0x30]` and args follow the struct offsets above. Every command in this report is valid on Lite (no productType-branch exclusions).

| export | kind/len | pair | args |
|---|---|---|---|
| aiSetHandTrackModeR | 0x0D | 49 30 | u8 @0x1c |
| aiSetHandTrackGimbalEnabledR | 0x0D | 57 30 | u8 bool @0x1c |
| aiGetHandTrackStateR | 0x0C | 4a 30 | none (resp: 5f+byte; 0xff variant with 0x0D) |
| aiSetHandTrackInitPosR | 0x24 | 4b 30 | 24B pos block @0x1c (scaled int16 path on Lite) |
| aiRstHandTrackInitPosR | 0x0C | 4d 30 | none |
| aiTrgHandTrackInitPosR | 0x0C | 4e 30 | none |
| aiSetTrackSpeedTypeR | 0x0D | 68 30 | u8 @0x1c |
| aiSetHorizontalOffset | 0x10 | 79 30 | float @0x1c |
| aiGetHorizontalOffset | 0x0C | 78 30 | none (resp float @0) |
| aiSetVerticalOffset | 0x10 | 7b 30 | float @0x1c |
| aiGetVerticalOffset | 0x0C | 7a 30 | none (resp float @0) |
| aiSetAutoOffset | 0x0D | 76 30 | u8 bool @0x1c |
| aiSetGestureCtrlR | 0x0E | 0d 30 | u8 sub=5 @0x1c, bool @0x1d |
| cameraSetGestureControlU | sel6 | field 0x520 | [2]=en,[3]=sub,[4]=bool,[5..6]=u16 |
| aiSetTrackingModeR (comparison) | 0x0D | 11 30 | u8 AiVerticalTrackType |
