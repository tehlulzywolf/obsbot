# Roadmap RE — items 10, 11, 13, 15 (libdev.dll exports)

Target: `C:\Program Files\OBSBOT Center\bin\libdev.dll` (x64, PDB libdev.pdb GUID 2ca512b4-4286-3796-eac9-e73cb000b3c2), REA doc `target-2486d1f3fe8b.dll`, Ghidra 12.1.4 pseudocode.
Date: 2026-11-09. Companion raw notes: `_notes_item10_11_13_15.md` (same dir).
All addresses are image-relative `0x180000000 + CSV RVA`.

## Protocol recap (used by every table below)

Setter-built request struct (per parent protocol, re-confirmed in decompiles):
- `u16 @+0x00` = "kind". GET = `0x0C`. SET kinds observed: `0x0D` (1-byte arg), `0x0E` (1-byte arg + reply-class), `0x10` (u32 arg), `0x11` (bool+int, other-product variants), `0x14` (u64/8-byte struct arg), `0x21` (0x15-byte packed struct), `0x3C` (0x30-byte struct). Pattern: byte0 = 12 (GET) else ≈ 13 + payloadLen; mirror the exact per-command value in the Rust port.
- `u16 @+0x0E` = pair-ID, `u16 @+0x10` = sub/family nibble (0x10 = normal camera family; 0x15–0x19 = TX-accessory sub-commands). Wire pair written as `[ID,sub]`.
- `u8 @+0x0A` = reply-length class; `u8 @+0x0B` = TX-type/dev selector; `u8 @+0x0C` = 0x0B (camera family tag; the common `local_*1d = 0xb00` constant = bytes +0xB=0, +0xC=0x0B).
- `u32 @+0x14` = copy of serial (dev+0x1668), `u32 @+0x18` = copy of dev_mode (dev+0x3c) — channel routing.
- args at `+0x1C`.
- Senders: `FUN_180007490(dev,req,reply_buf)` = AA GET; `FUN_180008180(dev,req,0,0)` = AA SET; `FUN_1800087f0(dev, 6, buf)` = **sel6** — AiMode selector-6 60-byte struct with `u16 field-id @buf[0..1]` and value bytes from buf[2] (this is the dev_mode 0 path); `FUN_1800085e0(dev, sel, buf)` = host-side cache/selector getter (no wire pair).
- Wire command/type bytes on the transport are obfuscated via lazy maps (`DAT_180218e28/e58/ed8/f08/ee8/ef8/f28`, built in `FUN_180100d00/f70/aa0/fff10`); the pair-IDs in this report are the decoded (de-obfuscated) values.

**dev_mode 0 (Tiny 2 Lite: dev+0x3c==0, serial 0, product dev+0x680==3):*U exports go through sel6; *R exports still use AA wire pairs.** Where a function branches on productType, the other-product values are listed; productType 0xB/0xC/0xD/0x10/0x12/0x13/0x16 are other products (Tail/Senso etc.).

---

## Item 10 — Audio

All audio exports below are **wire commands** (AA pair or sel6 field). None of the set/get pair is a pure Windows-endpoint host filter. The only host-side (no-wire) audio items are `cameraAudioDevPath` / `cameraAudioFriendlyName` (read host audio-endpoint strings via `FUN_1800085e0` selector 0x27/0x28) and the read-side cache for `Distance`/`MicDisabled` (host cache selector 0x25 — there is NO wire GET for those two; the value is app-side state).

| Export | Addr | Path | Wire / field | Args & reply |
|---|---|---|---|---|
| cameraSetAudioVolumeR | 0x1800F7EA0 | AA | kind 0x0E, pair [0x74,0x10] | byte@+0x1c = volume (0-100) |
| cameraGetAudioVolumeR | 0x1800F7F40 | AA | kind 0x0C, pair [0x73,0x10] | reply u16@0 = volume |
| cameraSetAudioAGCR | 0x1800F8180 | mixed | product4: kind 0x0D [0x77,0x10] byte=bool; products 0xB/0x16/0x10: pre-GET [0x78,0x10] then kind 0x11 [0x79,0x10] payload +0x1c b0=new, +2/+3=echo prev reply b1,b3; else (dev_mode 0) **sel6 field 0x0117** byte=bool | log string "cameraSetAudioAutoGainU" |
| cameraGetAudioAGCR | 0x1800F8A80 | AA | kind 0x0C, pair [0x78,0x10] | reply byte0 != 0 = AGC on |
| cameraSetAudioMicDisabledU | 0x1800F8410 | sel6 | field **0x011C**, value byte = **ENABLED** (= !micDisabled — inverted!) | no wire GET; host cache sel 0x25 |
| cameraSetAudioDistanceU | 0x1800F84C0 | sel6 | field **0x011B**, value byte = uchar distance level | no wire GET; host cache sel 0x25 |
| cameraSetMicrophoneDuringSleepU | 0x18002ECB0 | sel6 | field **0x0113**, value byte = flag | — |
| cameraSetAudioNoiseReduceR | 0x1800F8B50 | mixed | dev_mode0 && product∉{4,0xB,0x16,0x10}: **sel6 field 0x010A**, value = enabled ? level : 0; else kind 0x11 pair [0x7B,0x10], +0x1c b0=enabled, int@+0x1d=level | — |
| cameraGetAudioNoiseReduceR | 0x1800F8D20 | AA | kind 0x0C, pair [0x7A,0x10] | reply b0=enabled, int@1..4=level |
| cameraSetAudioVQETypeR | 0x1800F8010 | AA | kind 0x0D, pair [0x76,0x10] | byte = DevAudioVQEType |
| cameraGetAudioVQETypeR | 0x1800F80B0 | AA | kind 0x0C, pair [0x75,0x10] | reply byte = enum |
| cameraGetAudioAuxInputTypeR | 0x1800F8E00 | AA | kind 0x0C, pair [0x7C,0x10] | reply byte = enum |
| cameraSetAudioAuxInputTypeR | 0x1800F8ED0 | AA | kind 0x0D, pair [0x7D,0x10] | byte = enum |
| cameraSetAudioSourceR | 0x1800F8F70 | AA | kind 0x0D, pair [0x7F,0x10] | byte = source id |
| cameraGetAudioSourceR | 0x1800F9020 | AA | kind 0x0C, pair [0x7E,0x10] | reply 0x21 bytes → DevAudioInputSource verbatim (0x20B struct + byte +0x20) |
| cameraSetAudioSourceMuteR | 0x1800F9100 | AA | kind 0x0D, pair [0x81,0x10] | byte bool |
| cameraGetAudioSourceMuteR | 0x1800F91A0 | AA | kind 0x0C, pair [0x80,0x10] | reply byte bool |
| cameraGetSelectedAudioSourceR | 0x1800F9270 | AA | kind 0x0C, pair [0x82,0x10] | reply byte = selected source |
| cameraSetAudioCtrlStateU | 0x18002FBC0 | sel6 | field **0x0215**: [2]=AudioCtrlCmdType byte, [3]=value byte | — |
| cameraSetAudioAutoGainU | 0x18002FC70 | sel6 | field **0x0117** (same field as AGCR), value = bool | — |
| cameraSetAudioModeU | 0x18002FE80 | sel6 | field **0x0222**: [2],[3] = AudioMode struct 2 bytes | — |
| cameraTXSetAudioGainR | 0x1800FD480 | AA | kind 0x10, pair-ID@+0xe = 0x0B, sub@+0x10 = 0x16 (wire pair [0x0B,0x16]); +0xA=0x15, +0xB=DevTXType | int@+0x1c = gain |
| cameraTXGetAudioGainR | 0x1800FD520 | AA | kind 0x0C, pair [0x0B,0x17]; DevTXType @+0xB | reply int@0 = gain |
| cameraTXSetAudioMuteR | 0x1800FD5F0 | AA | kind 0x0D, pair [0x0B,0x18]; +0xB=DevTXType | byte@+0x1c = mute bool |
| cameraTXGetAudioMuteR | 0x1800FD690 | AA | kind 0x0C, pair [0x0B,0x19]; +0xB=DevTXType | reply byte = mute |
| cameraGetAudioSelectR / SetAudioSelectR | 0x1800FD9A0 / FDA70 | AA | same family; IDs not re-recorded this pass | — |
| get/setUSBAudioParamR | 0x1800FDB10 / FDBB0 | AA | same family; IDs not re-recorded | — |
| setTWSSoundMode | 0x1800FDEA0 | AA | same family; ID not re-recorded (last batch decompile failed) | — |
| cameraAudioDevPath / cameraAudioFriendlyName | 0x180001210 / 01260 | host-only | read host Windows audio endpoint path/friendly-name strings (selector cache 0x27/0x28) | no wire |

DevTXType enum: values pass through as one selector byte; the two TX accessories are {0,1} (TX-S / TX-R; exact labeling app-side).

## Item 11 — Background + Sleep

| Export | Addr | Path | Wire / field | Notes |
|---|---|---|---|---|
| cameraSetBgModeU | 0x18002F840 | sel6 | field **0x0105**, value byte = MediaBgMode | libdev passes the enum through; no enum-value switch exists in libdev (string search: no BgMode tables) — values are app-side (0..n: none/blur/color/custom per OBSBOT app convention, UNVERIFIED-BY-EYES) |
| cameraSetBgColorU | 0x18002F8F0 | sel6 | field **0x0110**, value byte = MediaBgModeColorType | pass-through |
| cameraSetBgEnableU | 0x18002F9A0 | sel6 | field **0x0111**, value byte = **!enable** (field semantics = BG DISABLED) | inversion trap |
| cameraSetDisableSleepWithoutStreamU | 0x18002EC00 | sel6 | field **0x0112**, value = bool | — |
| cameraSetAutoSleepTimeR | 0x1800FA0D0 | AA | kind 0x10, pair [0x91,0x10], u32@+0x1c = seconds | — |
| cameraGetAutoSleepTimeR | 0x1800FA170 | mixed | dev_mode 1/2: host cache dev+0xB80 (NO wire); else kind 0x0C pair [0x90,0x10], reply u32@0 | — |
| cameraGetScheduledSleepR | 0x1800FA260 | mixed | dev_mode 1/2: 0x30B cache dev+0xB84; else kind 0x0C pair [0x97,0x10] | reply unpack in struct note below |
| cameraSetScheduledSleepR | 0x1800FA3E0 | AA | dev_mode0: kind 0x21, pair-ID 0x98 (`local_186a = 0x10098`), payload 0x15B at +0x1c; dev_mode≠0: kind 0x3C, pair-ID 0x95, payload full 0x30B | — |
| cameraGetScheduledResumeR | 0x1800FA510 | mixed | dev_mode1/2: cache dev+0xBAC; else kind 0x0C pair [0x99,0x10] | — |
| cameraSetScheduledResumeR | 0x1800FA690 | AA | dev_mode0: kind 0x21 pair-ID 0x9A; dev_mode≠0: kind 0x3C pair-ID 0x96; payload = 21B packed (+0x1c: b0,b1, then param+0x1C,+0x18,+0x14,+0x10,+0x20,+0x24,+0x28) / 0x30B struct | log "set schedule resume, param: %d %d %d %d" |

**`aiSetBgScheduleSleepR` does not exist in the export table.** The schedule-sleep API is `cameraSet/GetScheduledSleepR` above (struct `DevScheduledInfo`, 0x30 bytes, same shape as ScheduledResume).

**DevScheduledInfo (0x30B) wire reply layout** (from `GetScheduledSleepR` unpack, verified `scheduled_sleep: %d %d %d %d %d %d`):
`r[0]`→+0x00 u32 (enabled), `r[1]`→+0x04 u32 (repeat/week-mask), `r[2..6)`→+0x08 u32 (hour), `r[6..10)`→+0x0C u32 (minute), `r[10..14)`→+0x10 u32, `r[14..21)`→ single bytes → +0x14,+0x18,+0x1C,+0x20,+0x24,+0x28,+0x2C (u32 each). The device reply is 21 tightly-packed bytes; the host struct pads to 0x30. The 0x21-payload SET sends bytes {+0,+1,+2..,+0x14-packed} — mirror the same order (decompile shows payload order: struct b0,b1 then +0x1C,+0x18,+0x14,+0x10,+0x20,+0x24,+0x28 for Resume; Sleep variant packs 2-13 from struct+8..; verify per-direction against a capture before shipping).

**Custom-bg image upload path:** there is **no** `BgImage`/`CustomBg`/`WallPaper` export or string in libdev.dll. The custom background image reaches the device through the generic file-transfer channel: `uploadFileToAppSd` 0x180104F50, `startFileUploadAsync(FileType)` 0x18010650, `devTransferringFileByMtp()` 0x18011320, `setFileUploadCallback` 0x18011B30 (plus `cameraUploadSrtConfigR` 0x180032DA0 as the pattern for small-config uploads). After the file lands, BgMode=custom (sel6 field 0x0105) activates it. New-file arrival on device is async event `FUN_180028590` "new file generate notify" (see item 15).

## Item 13 — Beauty / ImageStyle / Gamma

String search `eauty` in libdev.dll: **no match** (single RTTI-noise hit). There is no Beauty export and no beauty-related string: **beauty is a host-side (OBSBOT Center app) filter, not a libdev wire feature.** The device-side image tuning exports are:

| Export | Addr | Path | Wire / field | Notes |
|---|---|---|---|---|
| cameraSetImageStyleR | 0x1800F54F0 | AA | kind 0x10, pair [0x58,0x10], u32 = DevImageStyle | — |
| cameraSetImageStyleModeR (enum) | 0x1800F73A0 | AA | kind 0x10, pair [0x6A,0x10], u32 = DevImageStyleMode | log "set camera image style mode to %d failed" |
| cameraGetImageStyleModeR (enum) | 0x1800F7230 | AA | kind 0x0C, pair [0x69,0x10] | reply byte = enum |
| cameraSetImageStyleModeR (struct) | 0x1800F7300 | AA | kind **0x14**, pair [0x6A,0x10], u64@+0x1c = 8-byte DevLiveStreamModeSetting | — |
| cameraGetImageStyleModeR (struct) | 0x1800F7160 | AA | kind 0x0C, pair [0x69,0x10] | reply 8 bytes → DevLiveStreamModeSetting |
| cameraSetGammaModeR | 0x1800F6F30 | AA | kind 0x10, pair [0x66,0x10], u32 = DevGammaMode | — |
| cameraGetGammaModeR | 0x1800F6E60 | AA | kind 0x0C, pair [0x65,0x10] | reply byte = enum |
| cameraSetGammaUserModeR | 0x1800F3E20 | mixed | dev_mode0: **sel6 field 0x0026** with length-encoded high byte: buf[0]=0x26, buf[1]=(N+1)*2 (N≤0x16), buf[2]=mode, buf[3]=N, then pairs buf[4+2i]=X[i], buf[5+2i]=Y[i] via FUN_1800087f0(dev,6,..); dev_mode≠0: kind = 0x0C+(2+2N), pair [0x49,0x10], payload @+0x1c = {mode, N, (X[i],Y[i])×N} | log "set camera gamma user mode failed" |
| cameraGetGammaUserModeR | 0x1800F3C30 | mixed | dev_mode0: `FUN_1800085e0(dev, selector 0x10, buf)` host/selector getter, reply 8B → struct (+0 flag, +1 N, +2 X[], +0x18 Y[]); dev_mode≠0: kind 0x0C pair [0x48,0x10] | reply b0→+0, b1→+1 (clamp 0x16), X[i]=r[2+i]? decompile shows pairs r[1+2i]/r[0x18-?] — treat as (X[i],Y[i]) interleaved from r+1 |

GammaUserMode struct `DevGammaModeUserMode`: +0x00 u8 user-on flag, +0x01 u8 N (≤22), +0x02 u8 X[22], +0x18 u8 Y[22] (0x30 total).

## Item 15 — Status read-back

### aiGetAiStatusR (0x180019280) — AA GET pair **[0x13, S=3]** (request u32@+0xe = 0x30013, byte@+0xA = 3)
Request: kind 0x0C, reply-length class 3. Reply parsed per-product by three inline lambdas (LAB_1800190C0 serial==0; LAB_180019100 products 2,3,0xA,0xD,0xC,0x19; LAB_1800191E0 products 0xB,0x10,0x12,0x13,0x16 — decompiler cannot emit pseudo for the lambdas, only addresses). Success → `AiStatus` 0x30B copied to cache dev+0xCDc..0xD0B and logged `"AI Status: %d %d %d %f"`.

**AiStatus struct (0x30 bytes, offsets = struct offset = dev-cache offset − 0xCDc):**

| Off | Type | Meaning (from event-writer FUN_1800188c0, see below) |
|---|---|---|
| +0x00 | u8/bool | state flag bit0 (p4: payload+0x20≠0) — tracking/face active candidate |
| +0x01 | bool | bit1 |
| +0x02 | bool | bit2 |
| +0x03 | bool | bit3 (p4: payload+0x26) |
| +0x04 | bool | bit6 (p4: payload+0x23) — **sleep-state candidate** (UNVERIFIED-BY-EYES) |
| +0x08 | float | **zoom level** = u16 × 0.01 (1.00 = 1×) |
| +0x0C | u8 | p4: payload+0x28 / other: payload+0x24 |
| +0x0D | u8 | other products: payload+0x23 |
| +0x20 | u32 | raw AI-mode byte (p4: payload+0x1D) |
| +0x24 | u32 | p4: payload+0x27 |
| +0x28 | u32 | other: payload+0x1E |
| +0x2C | u32 | other: payload+0x1F |

Companion host-side state: dev+0x860 = sanitized mode enum (product4 map: payload+0x1D ∈ {1→1, 3→2, 4→3, 0xA→4, 0x1E→5, 0x14/15/16→0xFF, default 0}); dev+0x8B8 = QPC-derived ns timestamp of last status push.

### Async status events — dispatcher `FUN_180009450` (receiver path `FUN_180011750` → header parse `FUN_1800FFF10`)
Frame header (first 0x1C bytes) carries mangled (type,cmd); wire bytes are de-obfuscated through the lazy DAT maps (FUN_180100D00/F70/AA0). The raw status event **0x89 on the wire maps (via the obfuscation map — UNVERIFIED-BY-EYES) to decoded case (type 3, cmd 0x14)**, whose handler is:

**`FUN_1800188c0(dev, payload)` — AI/status push decoder (verified pseudocode, 2026-11-09).** Payload state bytes begin at `payload+0x1D` (0x1C-byte header before them):

- productType 4 branch: `+0x1D` = raw mode (→ cache +0x20 and dev+0x860 via the 1/3/4/A/1E→1/2/3/4/5, 0x14-0x16→FF map above); `+0x20` bool → AiStatus+0; `+0x21` → +1; `+0x22` → +2; `+0x26` → +3; `+0x23` → +4; **`u16@+0x24` × 0.01 → float zoom at AiStatus+8**; `+0x27` → +0x24; `+0x28` → +0x0C.
- all other products: **byte `+0x20` is the bitfield** (bit0→+0x00, bit1→+0x01, bit2→+0x02, bit3→+0x03, bit6→+0x04, bit4→+0x05-ish dev+0xCE1, bit5→dev+0xCE2); **`u16@+0x21` × 0.01 → zoom float**; `+0x1E` → +0x28; `+0x1F` → +0x2C; `+0x23` → dev+0xCE9; `+0x24` → dev+0xCE8.

So the Rust port parses status events as: header(0x1C) → mode byte @0x1D → (product4) individual bool bytes @0x20-0x23/0x26 + u16 zoom @0x24; (others) packed flag byte @0x20 + u16 zoom @0x21. Zoom is always `u16/100`. The sleep bit is flag-bit6 / payload+0x23 (candidate, needs device sighting); tracking/face flags are bits 0-3.

Other decoded async cases in the same dispatcher (for event-loop coverage):
- type1 cmd 0x12: `FUN_180028590` "new file generate notify" (custom-bg / file arrival; forwards file-info to dev+0x2288, event tag 0x82/0x7D8).
- type1 cmd 0x2C: `FUN_180011D00` (category map val 1-9→3, 0x11-0x1A→2, 0x21-0x2C→1 at dev+0x849).
- type3 cmd 0x6: time sync `FUN_180022740`.
- type3 cmd 0x64: `FUN_180014250`; 0x74/0x75: `FUN_180014310` (wake ConditionVariable dev+0x17E0 — sleep/wake handshake); 0x67: `FUN_1800178F0` "preset arrived"; 0x69: `FUN_1800185C0` pose/tracking struct (0x14 dwords → dev+0xD30).
- type6: `FUN_180105E40` "ap status notify" (Wi-Fi AP state), `FUN_180106770`.
- `cameraSetCameraStatusU` 0x1800FD8E0 (mixed; dev_mode≠0 sets camera-mode via kind 0x11 pair-ID 0x12, `local_187a=0x10012`) vs `cameraGetCameraStatusU` 0x1800FD9E0 (host-only: reads cached int at dev+0x2728).

## New sel6 field ids discovered this pass
0x0105 BgMode · 0x010A NoiseReduce · 0x0110 BgColor · 0x0111 BgDisabled(!) · 0x0112 DisableSleepWithoutStream · 0x0113 MicDuringSleep · 0x0117 AudioAGC/AutoGain · 0x011B AudioDistance · 0x011C MicEnabled(!) · 0x0215 AudioCtrlCmd · 0x0222 AudioMode · 0x0026+length-encoding GammaUserMode (getter side: selector 0x10 via FUN_1800085e0).

## Open items / caveats
- Wire 0x89 ↔ decoded (3,0x14) mapping and the sleep-bit semantics are inference from dispatcher structure + de-obfuscation maps: UNVERIFIED-BY-EYES (needs one captured status frame).
- DevScheduledInfo SET payload byte-order (Sleep vs Resume packing differ slightly in decompile) — verify with a live capture.
- `GetAudioSelectR/SetAudioSelectR`, `get/setUSBAudioParamR`, `setTWSSoundMode` pair IDs not recorded (decompile batch failed on last call); same AA family, trivial to finish.
- The three `aiGetAiStatusR` reply lambdas are inlined (LAB_ targets, not functions); GET-reply byte positions beyond cache offsets above are inferred from the cache/event writer, not the lambda bodies.
