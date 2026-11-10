# OBSBOT libdev.dll — per-command payload layouts (final decompile)

Source: `C:\Program Files\OBSBOT Center\bin\libdev.dll`
SHA-256: 2486d1f3fe8be947fe62f6392a1f24641607f5d3b0d865cd31b1eef18d254bd8
Method: Ghidra 12.1.4 (REA MCP) full decompile of the `Device::*` / `Camera::*`
export wrappers, 2026-11-06. All wrappers build an identical in-memory request
struct and hand it to the shared senders:

- `FUN_180008180(dev, req, ...)` = fire-and-forget / set path
- `FUN_180007490(dev, req, resp)` = sync get path (returns `short`, fills resp buffer)

## The universal request struct (what a Rust `build_request()` must emit)

Offsets below are into the stack struct each wrapper zeroes then fills
(`local_1828` family). The wire payload = bytes from +0x0C onward; bytes +0x00
and +0x09 feed the 12-byte frame header (length/nibbles — see framing notes in
the isolation report).

| off | size | field | notes |
|-----|------|-------|-------|
| +0x00 | u16 | `kind` | opcode-family selector baked into frame len/nibble fields: 0x0C=get-status style, 0x0D=simple set (no-SN), 0x0E=set (SN path), 0x10=timed/ACK set, 0x12=gimbal speed/rst, 0x13=get-with-args, 0x14=track-mode, 0x18=pos/speed ctrl, 0x19=speed ctrl variant, 0x20=preset trigger |
| +0x02 | u6 0 |
| +0x06 | u16 | 0 | |
| +0x08 | u8 | `chan` | transport/channel byte: 1, 2 or 3 |
| +0x09 | u16 | 0x0B00 | constant (feeds nibble-mapped header bytes 9-10) |
| +0x0B | u8 | 0 | |
| +0x0C | u32 | **cmd word** = `(cmd_set << 16) | cmd_id` ← payload[0:4] | |
| +0x10 | u32 | `dev+0x1668` | SN-present word (0 = first-claim/no-SN mode; switches framing — see gimbalRstPosR) |
| +0x14 | u32 | `dev+0x3c` | product type (4 = Tiny 2 Lite family branch) |
| +0x18 | ... | args | payload[4:] — layouts in the tables below |

Response buffer (`FUN_180007490`) mirrors the struct: args read back start at
+0x18 (i.e. response payload after its own 12-byte frame + 4-byte cmd word).

## Set 3 — AI / gimbal control (chan = 3)

| cmd | export (VA) | kind | payload args (+0x18) | notes |
|-----|-------------|------|----------------------|-------|
| 0x00 | aiSetGimbalSpeedCtrlR (0x180019830) | 0x19/0x18 | f32 ×3 (pan,tilt,speed) | no-SN (SN word==0): 3 f32 args; SN mode: different kind + scaled args |
| 0x04 | aiSetGimbalEulerAngleR (0x180019cb0) | 0x18 | f32 yaw, f32 pitch, f32 roll | absolute angles, deg |
| 0x0C | aiSetGimbalStop (0x180019b80) | 0x0C | none | stop all motion |
| 0x0B | aiGetGimbalStateR (0x180019e60) | 0x0C | req none; **resp**: 32 B `AiGimbalStateInfo` (u8×4 + f32×7: yaw/pitch/roll + targets) | |
| 0x0D | aiSetEnabledR no-SN (0x18001ab60) | 0x0E | u8 on/off | AI tracking enable |
| 0x62 | aiSetEnabledR SN (0x18001ab60) | 0x0D | u8 on/off | SN-mode variant, kind 0x0D |
| 0x11 | aiSetTrackingModeR (0x18001ac20) | 0x0E | u8 mode | `AiVerticalTrackType` (horizontal/vertical/full) |
| 0x6C | aiSetNoTrack (0x180020400) | 0x0E | u16 zone mask | exclude zones from tracking |
| 0x71 | aiSetAiTrackModeEnabledR (0x180020cc0) | 0x14 | u32 = (sel1<<16)|sel0 via mode table | mode 1→(0,0) 2→(4,0) 3→(3,0) 4→(2,0) 5→(1,0) 0xA→(0,1) 0xB→(2,1) 0xC→(1,1) |
| 0x72 | aiSetAiTrackModeEnabledR mode=0x1E (0x180020cc0) | 0x14 | no args | auto-group mode enable |
| 0x3E | aiTrgGimbalPresetR (0x18001fc20) | 0x20 | u32 preset_id | goto preset; U-path fallback = UVC ext subcode 7 |
| 0x37 | aiAddGimbalPresetR (0x18001f380) | 0x0C | `PresetPosInfo` packed: u32 id, f32×3 pos(×100), f32 zoom, f32×2 fov, u8 name_len(≤0x32), char name[] | product-4 clamps f32 fields to -1000.0 (0xC47A0000) |
| 0x35 | aiGetGimbalPresetListR (0x18001dfb0) | 0x0C | req none; **resp**: u8 count, then u8 id[count] | |
| 0x36 | aiGetGimbalPresetListR (products 4, B, 0x16, 0x10, 0x12, 0x13) | 0x0C | same | product-remapped cmd_id |
| 0x5B | aiSetGimbalPresetNameWithIdR | 0x0E | u16 id + name string | |

## Set 2 — raw gimbal control (chan = 2)

| cmd | export (VA) | kind | payload args | notes |
|-----|-------------|------|--------------|-------|
| 0x00 | gimbalGetAttitudeInfoR (0x180104600) / gimbalGetAllInfo (0x180104cc0) | 0x0C | req none; **resp**: 32 B: f32 roll, pitch, yaw + 20 B status | Tiny2Lite caches at dev+0xd0c..0xd2c |
| 0x02 | gimbalSpeedCtrlR (0x1801043c0) | 0x12 | i16 ×3 (×100): pan_spd, tilt_spd, roll_spd — args at +0x18..+0x1D | signed speed %, -1..1 → -100..100 |
| 0x03 | gimbalSetSpeedPositionR product<2 (0x180104990) | 0x18 | 6 × i16 (×100) at +0x18..+0x23: pos1..3 then spd1..3 order per locals layout (param_4..6 = speed?, param_1..3 = pos?) — verify with sniff | |
| 0x04 | gimbalSetSpeedPositionR products in bitmask 0x004D_181C (2,3,4,0xC,0x11,0x12,0x13,0x16) | 0x18 | same | |
| 0x01 | gimbalRstPosR SN-mode (0x1801042c0) | 0x12 | u8 mode (0x7D for products B/0x16/0x10, else 0) + u16 0 | reset position |
| 0x06 | gimbalRstPosR no-SN (0x1801042c0) | 0x0D | none | |
| 0x08 | gimbalGetInfoR (0x180104bd0) | 0x13 | req: u32+u16+u8 from GimbalInfo (angle limits query); **resp**: u32, u16, u8 (min/max angles) | |

Inner remap layer (cmd_tables.json) maps set 2 keys 0-6 → wire codes 1,4,6,0xA,0x22,0x23
for the phone/cloud protocol; local USB set 0 passthrough still applies — see
isolation report for 0xFFF sentinel rule.

## Set 1 — camera basic (chan = 1)

| cmd | export (VA) | kind | payload args | notes |
|-----|-------------|------|--------------|-------|
| 0x91 | cameraSetAutoSleepTimeR (0x1800fa0d0) | 0x10 | u32 seconds | auto-sleep timer |
| 0x02 | cameraSetDevRunStatusR (0x18002e480) | 0x10 SN / 0x0E no-SN | SN: u32 (status!=1 ? 1:0); no-SN: u16 (status,1) | sleep/wake run status |
| 0x17 | cameraGetBootStatus (0x180030580) | 0x0C | req none; **resp**: u8 booting_flag, u32 ×3 (progress/version fields) | products 1,2 serve from cache (dev+0xc4c..0xc58, fed by status notify) |
| 0x34 | cameraSetZoomStopR (0x180031b40) | 0x0C | none | zoom stop (vendor path) |

## Zoom & pan/tilt = mostly UVC, not vendor HID

From cameraSetZoomAbsoluteR (0x180029ba0), cameraSetZoomRelativeR (0x180029b10),
cameraSetPanTiltAbsolute/Relative (0x180011e50 / 0x180011c10) and
cameraSetAiModeU (0x18002fb10):

- Pan/tilt: sent as standard **UVC PTZ class controls** (CameraControl_Pan/Tilt,
  KSPROPERTY set on the camera filter) plus UVC-ext node 0x409 for relative steps.
- Zoom absolute/relative: **UVC CameraControl_Zoom** (vtable put, range from
  extension node); vendor 0x1_0031 (cameraGetZoomAbsoluteR) only for SN-mode
  models, resp = u32 zoom_pos (+ min/max u16s).
- AI mode: **UVC extension unit** setter `FUN_1800087f0(dev, subnode=6|7, payload)`;
  presets go through ext subnode 7.

Linux consequence: standard UVC pan/tilt/zoom come free via V4L2
(`uvcvideo`); only the vendor-HID commands above need hand-rolled frames.

## UVC extension-unit payloads (FUN_1800087f0 path, for the `*U` APIs)

| subnode | use | payload sample |
|---------|-----|----------------|
| 6 | cameraSetAiModeU | u8[4] = {0x216&0xFF, 0x02, mode, arg} (0x0216 word + 2 args) |
| 7 | cameraTrgGimbalPresetU / cameraAddGimbalPresetU | byte0 = op(7?), preset_id u8; add: id + f32 pos×100 (i32) + zoom + name ≤0x32 B |

## Response/notification plumbing

- Sync gets: short-timed request/ack via FUN_180007490 (see struct mirror above).
- Async: `setDevStatusCallbackFunc` (0x180011600) registers std::function
  (void*, void const*) — device status frames (boot progress, SN word,
  gimbal-state pushes) land here. Rust equivalent: an input-report reader
  task dispatching on cmd word.
- wakeUpDevice (0x180119ec0) is NOT a wire command: it latches pending
  (sn, timeout, callback) into the Devices singleton; the actual wake rides
  cameraSetDevRunStatusR / reconnect flow.
- connectDev/getDevList/start: C++ object lifecycle (hid enumeration, shared_ptr
  bookkeeping) — replace wholesale with Rust `hidapi`/`hidraw` enumeration; no
  protocol content.

## Rust port recipe (concrete)

1. Open VID_3564 PID_FEF9 via hidraw; read/write HID reports (report 0 =
   `[subcode][frame]`, frame = 12 B header + payload — see isolation report).
2. Frame builder: magic 0xAA, len, seq++, nibble-mapped bytes 9-10, wire-cmd
   word at byte 10 = `(setcode[cmd_set]<<12) | ...`; set 0 local path sends
   wire-cmd 0x0FFF with true `(set<<16|cmd)` u32 as payload[0:4].
3. Implement commands as `(kind, cmd_set, cmd_id, args)` tuples from the tables
   above — pan/tilt/zoom first via V4L2 UVC; vendor HID for gimbal presets,
   speed ctrl, tracking enable/mode, auto-sleep, boot status, preset list.
4. Validate each command with one sniffed exchange (the 0x0B00 constant and the
   kind-byte semantics in the header are the only fields needing live confirmation).
