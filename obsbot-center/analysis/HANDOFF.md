# OBSBOT Tiny 2 Lite — reverse-engineering HANDOFF (roadmap complete)

Target: OBSBOT Tiny 2 Lite (`VID_3564 PID_FEF9`), product type **3** ("Tail2Lite").
Port: Rust, Linux/WSL, kernel `uvcvideo` + V4L2 + `UVCIOC_CTRL_QUERY`, egui panel
(`obsbot-core/`, panel = `examples/panel.rs`, running via `run_panel.sh`).
Source of truth for all of the below: decompilation of
`C:\Program Files\OBSBOT Center\bin\libdev.dll` (Ghidra; program doc
`target-2486d1f3fe8b.dll`, image base 0x180000000) + live USB captures.
Eye tests on the panel override a decompile guess when the two disagree.

## Field status (2026-11-09, live Tiny 2 Lite over usbip)

- Human tracking follows. The panel writes selector 6 field `0216` as work mode
  2 and target type 1, then tracking enable `0D 30`, then human type `71 30`.
- Hand tracking follows. On is selector 6 field `0216` as work mode 3 and target
  type 0, then `49 30` byte 1, then gimbal follow `57 30`. Off is byte 0, gimbal
  follow off, and work mode 0.
- Left and Right do not select a hand. On product type 3, Center's handler for
  that button pair sends byte 1 for one index and byte 0 for the other. Byte 0
  is off. The pills in this panel only record which one is lit.
- Yaw reverse works on the pad. The `65 30` acknowledgement does not invert an
  absolute pan. The panel negates the pan stick while the switch is on.
- Orientation `2E 10` values 0 through 3 were acknowledged and the preview
  stayed upright.
- The preview is 720p30. A 60 fps preview on this usbip link has dropped the
  camera while a selector 6 write was in flight, so the panel pauses the stream
  around those writes. A separate transport on the same camera completed many
  selector 6 writes during 720p60, so a dropped link is not proof that every
  selector 6 write is fatal.

Detailed per-group reports (read these for pseudocode-level evidence):
- `roadmap_items_1_2_3_9_findings.md` — face focus/exposure, HDR, mirror/rotation, zoom
- `roadmap_re_item4_item6.md` — resolution/framerate, tracking mode + target enums
- `roadmap_re_item5_item14.md` — hand tracking, track speed/offsets/gestures
- `roadmap_re_item8_item12_factory.md` — presets, yaw reverse, factory reset
- `roadmap_re_item10_11_13_15.md` — audio, background/sleep, beauty, status events

## 0. Protocol base (do not re-derive)

- Vendor channel = UVC Extension Unit GUID `{9a1e7291-6843-4683-6d92-39bc7906ee49}`,
  bUnitID 2, `wIndex 0x0200`. XU selector 2 = AA frames (60 B, `GET_LEN`→`3c 00`),
  selector 4 = raw pan/tilt blocks, selector 6 = AiMode struct writes,
  selector 0x25 / 0x27 / 0x28 = host-side caches. EP `0x86` pushes `0x89` async events.
- AA frame: `[0]=AA | [1]=type | [2]=seq | [4..5]=len | [6..7]=request CRC (frame CRC
  skips these) | [9]=ftype/fl nibbles | [10..11]=pair | [12]=payload len | [16..]=payload`.
  Reply type `29`; async `89`. Request CRC at 6..7, split-table crc stored LE.
- Pair rule: internal command word `0xS_ID` -> wire bytes `[ID_low, S<<4]`.
  S=1 family (`10`) = camera/media commands; S=3 family (`30`) = AI/gimbal commands.
- Command staging struct passed to the sender (`FUN_180008180` -> builder
  `FUN_180100450`): `+0x00` u16 total wire length, `+0x0a` ftype-src (3 = AI/gimbal,
  1 = factory reset), `+0x0e` u16 command ID, `+0x10` u16 S, `+0x14` serial word
  (`dev+0x1668`, **Lite=0 -> no-serial builder path**), `+0x18` dev_mode (`dev+0x3c`,
  **Lite=0**; gate: UVC channel open + total len ≤ 0x3c), `+0x1c..` args copied verbatim
  to payload. Seq/ack fields are auto-filled. The earlier "kind 0x0C/0x0D/0x10" notes
  = struct **lengths**, not wire bytes. **No sub_ck checksum on this branch** —
  sub_ck exists only on the legacy type-0x21 preset branch (which the working Rust
  preset-goto uses: ftype 0x21, fl 04, pair 043b).
- Fire-and-forget: send errors only queue-full (−3, ≥0xA00 pending), UVC closed (−7),
  len > 0x3c (−5). Success = type-29 ACK echo of the pair (pending-ack tables).
- "U"-suffixed exports route through `FUN_1800087f0(dev, 6, buf)` = AiMode **selector-6**
  60-byte write: `buf[0..1]` = u16 LE **field id**, value bytes from `buf[2]`.
  Known field ids: `0x0100` MediaMode, `0x0101` WDR/HDR, `0x0103` FaceAE,
  `0x0105` BgMode, `0x0110` BgColor, `0x0111` BgDisabled (**inverted!**),
  `0x0112` DisableSleepWithoutStream, `0x0113` MicDuringSleep, `0x0117` AudioAGC,
  `0x0118` LED ctrl, `0x011B` MicDistance, `0x011C` MicEnabled (**inverted, = !disabled**),
  `0x0215` AudioCtrlState, `0x0216` AiWorkMode+TrackType, `0x0222` AudioMode,
  `0x0520` GestureControl, `0x1021` FieldView (4×u32), `0x0119` BootMode (packed nibbles).
- V4L2 path (`am_cam_ctrl` vtable: +0x18 QUERYCTRL, +0x20 S_CTRL, +0x28 G_CTRL) is
  what zoom uses; already implemented in `src/linux_kernel.rs`.
- Product type at `dev+0x680`: 2=Tiny2, 3=Tail2Lite (us), 4=TailAir, 0xB=Tail2,
  0xC=TinySE, 0x10=Tail2s, 0x12=Tiny3, 0x13=Tiny3Lite. Branches on other products
  never fire on the Lite.
- `is4K` (`0x1800100D0`) returns **true** for `Obsbot_ty2lite` — 4K capable.

## 1–3, 9. DONE + wired in the panel (details in findings file)

Face focus `03 10`, AF mode `5F 10`/`5E 10`, exposure mode `47 10`/`46 10`, HDR via
sel6 `0x0101` (AA fallback `14 10`), mirror/flip `2E 10`/`2D 10`, FaceAE sel6 `0x0103`,
MediaMode sel6 `0x0100`, FieldView sel6 `0x1021`. GETs `5E/46/2D 10` are **silent** on
Lite firmware — UI must not block on them.
**Zoom resolved**: Center = QUERYCTRL+G_CTRL `CID_ZOOM_ABSOLUTE` (0x9A0201|3<<16),
label `1+(cur−min)/(max−min)` (Lite has no x2.5/x3 branch). Measured: min 0 max 100
step 1 def 0. Set = S_CTRL with flag=2. `aiSetCameraZoomRatioR` (`34 30` f32+i32) is
AI auto-zoom only. Panel uses the Center formula verbatim.

## 4. Resolution / frame rate
- `cameraSetUvcModeR 0x1800FDF40` -> pair `B8 10`, u32 DevUvcMode passthrough @+0x1c
  (domain unknown, UNVERIFIED-BY-EYES). `cameraGetUvcModeR 0x1800FE470` -> `B9 10`,
  1-byte reply.
- `cameraSetRecordResolutionR 0x180030800` -> pair `1A 10`, u32 DevVideoResType
  passthrough (id→(w,h,fps) mapping unknown). Also `23 10` KCP preview res
  (`0x180030F60`), `24 10` NDI/RTSP res (`0x180031000`).
- **No fps export exists** — frame rate is negotiated host-side via UVC capture
  formats. The panel preview is 720p30. Capture restart is V4L2
  STREAMOFF, then S_FMT, then STREAMON. No vendor command sets the preview rate.
- Adjacent: main-video bitrate `cameraSet/GetMainVideoBitrateLevelR`
  `0x180030CB0`/`0x180030B60`.

## 5. Hand tracking (separate AA family, NOT a tracking_type value)
- `aiSetHandTrackModeR 0x18001b570` -> `49 30`, one byte. Byte 1 follows either
  hand. Byte 0 turns it off. Center's Left/Right pair on product type 3 sends
  those two values; it is not a side filter. Bytes 3, 4, and 5 occur in other
  switch cases and are not wired.
- `aiSetHandTrackGimbalEnabledR 0x18001bad0` -> `57 30`, bool.
- `aiGetHandTrackStateR 0x18001ad90` -> GET `4A 30`; reply = 5 floats + 1 byte→float
  (init-position info).
- Cluster: get init pos `0x18001b0b0`, set init pos `4B 30` (24 B compact:
  u32 index + 8×int16 = float×100), reset `4D 30`, trigger `4E 30` (`0x18001b4e0`).

## 6. Tracking target types + framing
- **Correction**: this build has NO `cameraSetTrackingTypeR`/`GetTrackingTypeR`
  exports (the old address 0x18002FD50 lands inside `cameraSetLedCtrlU`, sel6 field
  `0x0118`). The live-confirmed "Human=1" toggle maps to **sel6 field `0x0216`**
  (`cameraSetAiModeU 0x18002FB10`: buf[2]=AiWorkModeType, buf[3]=target type) and/or
  `aiSetAiTrackModeEnabledR`.
- `aiSetAiTrackModeEnabledR 0x180020CC0` -> pair `71 30`, two u32: category (ids 1–5
  → 0, ids 0xA–0xC → 1) + sub. Type map: 1=Human(live)->(0,0); 2->(0,4); 3->(0,3);
  4->(0,2); 5->(0,1); 0xA->(1,0); 0xB->(1,2); 0xC->(1,1); 6–9/default = no frame.
  disable→`70 30` `aiDelSelectedTargetR` (0x180020750) or `73 30` `aiDelAutoGroupModeR`
  (0x1800207E0). Auto-group enable = `72 30` (id 0x1E).
- `aiSetTrackingModeR 0x18001AC20` -> `11 30`, u8 AiVerticalTrackType (no names found).
- `aiSetWorkModeR 0x18001ACC0` -> `63 30`, 2 bytes (main mode, sub mode).
- `aiSetBootModeU 0x18002FDD0` -> sel6 `0x0119`, byte=(workMode<<4)|subMode.
- Framing candidates: `aiSetTargetViewTypeR 0x180021410` -> `84 30` u32;
  `aiSetTargetZoomTypeR 0x180021370` -> `83 30` 2×u32;
  `aiSetSelectedTargetR 0x1800212C0` -> `82 30`, 24 B (DevTargetSelection, floats+id).
  No naming strings in libdev — Center's UI strings live in `en-US.ini`; framing label
  mapping needs eyeball test.

## 7. Click-person-to-track
- `aiSetSelectTargetByPos` / `aiSetSelectTargetByBox` (addresses in item4/6 + item5/14
  reports; coords are in **video-frame space**, normalize before sending). Closest wire
  sibling: `82 30` DevTargetSelection (24 B: 4×u32 + u64 — floats for x/y/w/h + target id;
  exact field order UNVERIFIED-BY-EYES — confirm with one capture when wiring UI).

## 8. Presets (Lite = dev_mode 0 branch)
- List: `aiGetGimbalPresetListR 0x18001DFB0` -> GET `35 30`; reply u8 count + ids.
- Add: `aiAddGimbalPresetR 0x18001F380` -> `37 30`, wire 24 B: product-3 pack = raw f32
  `[id, pose floats from PresetPosInfo +0x0c,+0x04,+0x08,+0x10,+0x14]` (axis order
  unverified). Add also stores name host-side.
- Delete: `aiDelGimbalPresetR 0x18001F860` -> `38 30`, u32 id first.
- Update: `aiUpdGimbalPresetR 0x18001F9E0` -> `39 30` (param2=false; Lite): 18 B
  **compact ×100 int16 pose pack** + trailing u32 id (id at END, unlike delete);
  param2=true -> `41 30`, 24 B float pack + id.
- Rename: `aiSetGimbalPresetNameWithIdR 0x18001F210` -> `3D 30`: u32 id @wire+0x0c,
  then **raw UTF-8 name bytes, NUL-terminated**, ≤47 B name (frame ≤0x3c). No checksum.
- Goto (modern): `aiTrgGimbalPresetR 0x18001FC20` -> `3E 30`, u32 id + 4×f32 1.0f —
  Lite acceptance UNVERIFIED (panel's working goto uses legacy 0x21/043b branch).
- Boot position: get `08 30` (`0x18001A530`, serial-0 parser), set `07 30`
  (`0x18001A230`, Lite: 4×f32 at wire+0x0c, no id), trigger `0A 30` (`0x18001AA20`,
  bare 0x0c frame on Lite — bool ignored), reset `09 30` (`0x18001AAD0`).

## 10. Audio (wire commands on dev_mode 0 unless noted)
- Volume set `74 10` byte 0–100 (`0x1800f7ea0`); get `73 10` u16 reply (`0x1800f7f40`).
- AGC set: Lite path = **sel6 field `0x0117`** bool (`cameraSetAudioAutoGainU`
  `0x18002fc70`); get `78 10` byte0 (`0x1800f8a80`).
- Noise reduce set: Lite = **sel6 field `0x010A`** (enabled?level:0) (`0x1800f8b50`);
  get `7A 10` b0=enabled + int level (`0x1800f8d20`).
- Mic disabled set = sel6 `0x011C` value=**ENABLED=!**disabled (`0x1800f8410`, no wire
  GET); mic distance = sel6 `0x011B` level byte (`0x1800f84c0`); mic-during-sleep =
  sel6 `0x0113` (`0x18002ecb0`); audio ctrl state = sel6 `0x0215` (cmd byte+value);
  audio mode = sel6 `0x0222` 2 bytes.
- VQE type set `76 10`/get `75 10`; aux input `7D 10`/`7C 10`; source set `7F 10`/get
  `7E 10` (0x21 B struct); source mute `81 10`/`80 10`; selected source `82 10`.
- TX accessory family (`0B` pair-id, sub `16..19` gain/mute set/get) — no TX accessory
  on Lite, skip.
- `cameraAudioDevPath`/`cameraAudioFriendlyName` = **host-only** Windows endpoint caches.

## 11. Background + sleep
- BgMode = sel6 `0x0105` (MediaBgMode value app-side), BgColor = sel6 `0x0110`,
  BgEnable = sel6 `0x0111` with **INVERTED semantics (value = disabled)** — trap.
- DisableSleepWithoutStream = sel6 `0x0112` bool.
- Auto-sleep timer: set `91 10` u32 seconds (`0x1800fa0d0`), get `90 10` (`0x1800fa170`).
- Scheduled sleep/resume: `cameraSetScheduledSleepR 0x1800fa3e0` -> dev_mode 0: pair-id
  `0x98`, 21 B packed struct; get `97 10` (`0x1800fa260`) -> DevScheduledInfo 0x30 B
  (enabled, week-mask, hour, minute, ...; byte order needs one live capture).
  Resume set `0x9A` 21 B / get `99 10`. (`aiSetBgScheduleSleepR` does not exist.)
- Custom bg image: no dedicated export — generic file transfer
  (`startFileUploadAsync`/MTP `devTransferringFileByMtp`), arrival = async type1 cmd
  `0x12` "new file generate notify" (`FUN_180028590`).

## 12. Gimbal yaw-dir reverse
- `aiSetGimbalYawDirReverseR 0x180020220` -> **`65 30`** on product 3, plain bool byte
  (0/1). `66 30` is for products outside the Lite gate. No separate tilt-reverse call
  found — yaw only.

## 13. Beauty
- **No Beauty export/string in libdev** — Center's beauty is a host-side OBS filter
  (`rm.dll`). Device-side analogs: `cameraSetImageStyleR 0x1800f54f0` -> `58 10` u32;
  ImageStyleMode enum `6A 10`/`69 10`; LiveStreamModeSetting struct 8 B `6A 10`;
  Gamma mode `66 10`/`65 10`; **user gamma curve** `cameraSetGammaUserModeR 0x1800f3e20`
  -> dev_mode 0: sel6-style write `buf[0]=0x26, buf[1]=(N+1)*2, buf[2]=mode, buf[3]=N,
  then (X[i],Y[i]) byte pairs` (N≤0x16) — gamma IS a wire command.

## 14. Track speed / offsets / gestures
- `aiSetTrackSpeedTypeR 0x180021220` -> `68 30` u8 (slow/normal/fast candidate, unverified).
- `aiSetHorizontalOffset 0x1800209e0` -> `79 30` **f32 LE**; get `78 30` f32 reply.
  `aiSetVerticalOffset 0x180020b50` -> `7B 30` f32; get `7A 30`.
  `aiSetAutoOffset 0x180020870` -> `76 30` u8 bool.
- Gesture master: `aiSetGestureCtrlR 0x180018f20` -> `0D 30`, args [+0x1c]=5 (sub-cmd),
  [+0x1d]=bool. Detailed: `cameraSetGestureControlU 0x1800f87a0` -> sel6 `0x0520`
  (enable, sub/mode, bool, u16). `aiGetGestureModeR` absent.
- Zone track deferred: `aiSetLimitedZoneTrack*`.

## 15. Status read-back (the big one for UI sync)
- `aiGetAiStatusR 0x180019280` -> GET `13 30`; reply -> AiStatus 0x30 B:
  `+0..+0x04` 5 bools, `+0x08` f32 zoom (u16×0.01), `+0x0C/+0x0D` u8, `+0x20` raw mode,
  `+0x24..+0x2C` u32s. Product-3 parser = lambda `LAB_180019100`.
- Async status event: raw `0x89` frame decodes to type 3 / cmd `0x14`, handler
  `FUN_1800188c0` via dispatcher `FUN_180009450`. Payload after 0x1C header:
  mode byte (+0x1D product-4 layout; map 1->1,3->2,4->3,0xA->4,0x1E->5,0x14/15/16->0xFF),
  bools (+0x20..+0x22,+0x26) -> AiStatus+0..+4 (bit6/+0x23 = **sleep candidate**),
  **u16@+0x24 × 0.01 = zoom**, tracking/face = bits 0–3 of +0x20 (non-product-4 layout
  at +0x20/+0x21). Raw wire goes through runtime-obfuscated nibble maps
  (`DAT_180218e28/e58/ed8/ee8/ef8/f08/f28`) — identity assumption needs ONE captured
  0x89 frame to confirm before strict parsing.
- Dispatcher sibling events worth parsing: type3/`0x67` preset-arrived,
  type3/`0x69` pose/tracking (0x14 dwords), type3/`0x74-0x75` wake condition-variable
  (sleep/wake handshake), type1/`0x12` new-file notify, type1/`0x2C` category,
  type6 AP status.
- `cameraGetCameraStatusU` = **host cache only** (no wire).

## Factory reset (payload CONFIRMED)
- `cameraSetRestoreFactorySettingsR 0x18002E3E0` -> pair **`01 10`** (S=1, ftype-src 1),
  frame len 0x0D, **payload = one byte 0x01** (the confirm arg). Lite = no-serial path,
  no product gate. Companion `cameraSetFactorySettingsR 0x1800FA950` = same pair, **no
  payload**. No client timeout — watch for device disconnect/ACK loss after send.
  UI must double-confirm per roadmap.

## Live-port field notes (from the running WSL panel, 2026-11-09, second AI)
- **Sel6 "second-write kill" is NOT camera/usbip — it's the port's host stack.**
  Repro'd three ways in `obsbot-core/examples/` against this exact camera over
  this exact usbip link: `repro_sel6` (3x back-to-back sel6 SETs), `repro2`
  (sel6 writes WHILE MJPEG 720p60 stream active, no GET pull), `repro3_fields`
  (18 consecutive SETs covering 17 distinct fields incl. mic/AGC/NR/FaceAE,
  stream live throughout): **18/18 OK, zero link drops, `dmesg` clean** — only
  the benign `-104` unlink bursts that always appear on video-node open/close
  (confirmable by epoch-matching `MARK ... epoch=` lines the examples print).
  A REAL kill looks different in dmesg: EPIPE/ETIMEDOUT in uvcvideo completion
  handlers + usbip `connection closed` = the Windows side removed/reset the
  device. Prime suspect on the port: raw-usbfs/EP0 control transfers racing
  `uvcvideo`'s own EP0 traffic (two control paths on one endpoint -> STALL ->
  Windows device reset -> socket closed). Fix recipe: route ALL XU traffic
  through `UVCIOC_CTRL_QUERY` on the video node, serialized in-kernel, never a
  second direct-usbfs path to the same device. Golden capture context stands:
  Center itself only ever *GETs* sel6 (18 GET/GET_LEN, ZERO SETs) and writes
  everything through sel2 AA + sel4.
- **Selector-6 has no readable state on this firmware.** `GET_CUR` sel6 returns
  a STATIC 60-byte snapshot (`27 00 01 02 00 00 00 00 00 03 78 00 00 01 01 00
  00 00 00 00 01 00 21 00 00 01 0b 03 00 01 55 3c 00 03 10 00 ...00`) — byte
  for byte identical before and after 18 writes AND after a sentinel write
  sweep (`examples/layout_diff.rs` sentinels 0x11/0x22/0x33/0x44,0x55/0x66..0x99
  into 0112/0117/011C/0216/1021: zero diff). Consequences: (a) the type-29 ACK
  does NOT prove a sel6 write did anything — eyes or `0x89` events are the only
  ground truth; (b) the `0216` work-mode byte cannot be pinned by readback —
  pin it by behavior (71 30 + gimbal follow) or by a fresh Windows USBPcap
  capture with AI-tracking toggled IN Center (tx9.json was boot-only, so it
  proves nothing about tracking-era writes).
- Device state left by the test sweeps (restore in UI if needed): auto-sleep
  disabled (`0112`=1), AGC on (`0117`=1), mic on (`011C`=1), NR off (`010A`=0),
  HDR off (`0101`=0), FaceAE off (`0103`=0), `0216`=[wm 0, type 1], FieldView
  `1021`=four 1s.
- Consequence (revised 11-09 pm): sel6 writes are NOT blocked by the link —
  they are blocked by **verifiability**: ACK is a lie, readback is static, so a
  sel6 field can only be tested by watching physical behavior. mic (`011C`),
  AGC (`0117`), noise (`010A`), FaceAE (`0103`), bg fields (`0105/0110/0111`),
  gesture (`0520`) need eye-tests or `0x89` events, not more plumbing.
  AA alternatives to A/B: HDR `14 10` (probe-ACKed), noise-reduce AA `7B 10` after
  GET `7A 10` (product-4 form, may be ignored), AGC AA `79 10` after GET `78 10`.
- `13 30` aiGetAiStatus GET: **silent** on this firmware (consistent with other
  silent GETs `5E/46/2D 10` — Lite answers SETs, not status GETs).
- Mirror/flip `2E 10`: ACKed, no observable preview change (UVC node has no mirror
  ctrl; physical-mount effect unwatched). Panel toolbar rotate/mirror remain
  preview-only transforms.
- usbip reality: **30 fps is what survives** the vhci link on this host, despite
  the 60 fps descriptor — preview rate is effectively host-negotiated.
- HDR note for the port: libdev's dev_mode-0 path is sel6 `0x0101`; the AA pair
  `14 10` is the other-products fallback and was probe-acknowledged here. Which
  path actually tonemaps has not been confirmed by eye.

## Remaining UNVERIFIED-BY-EYES (needs the user + live camera, not more RE)
1. HDR tonemap visible? (already panel-wired; awaiting user test)
2. Orientation `2E 10` values 0–3 were acked and the MJPEG stayed upright, so the
   domain still has no visible mapping.
3. AF mode pills (`5F 10`) and AE ▸ (`47 10`) observed reactions -> name the enums.
4. Framing labels 1–4 (`84 30`) + target types 2–5/0xA–0xC (`71 30` map above).
5. Track speed `68 30` 0/1/2 -> slow/normal/fast? Offsets `79/7B 30` float units.
6. Gesture `0D 30` sub 5. Hand on/off is settled (byte 1 / byte 0); see the field status.
7. `3E 30` modern preset-goto accepted on Lite? 18 B ×100 update pack round-trip.
8. Scheduled-sleep 21-byte pack order. Sleep background, mic distance, status light,
   and brightness were exercised live and the link stayed up.
9. One captured `0x89` frame to pin the obfuscation-map nibbles for event parsing.
10. `B8 10`/`1A 10` mode/resolution domains (only matter if the preview leaves 720p30).

## Linux-app backlog (unchanged, not USB)
virtual camera; real video recording; format switch without dropping control conn;
factory reset UI (payload now confirmed, pair `01 10` + byte 0x01); firmware update
(DevUpgrade class — still deferred).
