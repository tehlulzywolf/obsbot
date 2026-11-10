# libdev.dll — Roadmap item 4 (resolution / frame rate) & item 6 (AI tracking mode / target types)

Target: `C:\Program Files\OBSBOT Center\bin\libdev.dll` (PDB libdev.pdb, GUID 2ca512b4-4286-3796-eac9-e73cb000b3c2), Ghidra 12.1.4 via REA. Image base 0x180000000. Names below come from the shipped PDB symbols (export RVAs cross-checked against `libdev_exports.csv`).

Protocol recap (established earlier, re-confirmed by packer `FUN_180100450` @0x180100450 this session):
- Setter/GET exports fill a stack frame struct, then call AA senders `FUN_180008180` (set) / `FUN_180007490` (get-with-reply).
- struct+0x00 = kind byte (observed values: **0x0C** = GET / no-arg, **0x0D** = SET 1 payload byte, **0x0F** = SET 3 bytes, **0x10** = SET 4-byte u32, **0x14** = SET 8 bytes (two u32), **0x24** = SET 24-byte struct). Kind correlates with payload size; earlier live captures interpreted 0x0D as the no-serial SET_SIMPLE path and 0x10 as SET_ACK — either way Lite (dev+0x3c==0, serial word dev+0x1668==0) emits exactly these same struct frames because **none of the exports analyzed here contain any product-type branch** (no dev+0x680 check anywhere in them).
- struct+0x0D word = 0x0B00, struct+0x0E u16 = 0xS_ID → wire bytes 10–11 = `[ID low byte, S<<4]` (scrambled via lookup-XOR tables in `FUN_180100450`). Pair notation below uses that mapping.
- struct+0x14/0x10 = serial word (dev+0x1668) and dev_mode (dev+0x3c) — embedded per established rule; payload starts at struct+0x1C = wire offset 0x0C.
- struct+0x1E sub-flag: **3 for all ai\* exports, 1 for camera\* exports** (purpose not established; constant per family).

`is4K` @0x1800100D0 is **not** a resolution check — it is a product-name capability test: returns **true** for `Obsbot_tiny_2`, `Obsbot_tiny3`, `Obsbot_tiny3lite`, `Obsbot_tailair`, `Obsbot_tail2`, `Obsbot_cm104`, **`Obsbot_ty2lite` (Tiny 2 Lite = 4K-capable)**, `Obsbot_meet2`, `Obsbot_meetflip`, `Obsbot_tail2s`; false for `Obsbot_tiny_2P` (unless model match), `Obsbot_tinyse`, `Obsbot_meet`, `Obsbot_meetse`.

---

## Item 4 — resolution / frame rate

### cameraSetUvcModeR @0x1800FDF40 (ordinal 400, rva 0xFDF40)
- Path **AA**, kind **0x10**, ID word **0x100B8** → wire pair **`B8 10`**.
- Arg: `DevUvcMode` u32 LE at struct+0x1C (4 bytes). Pure pass-through — no switch, no validation, no product branch.
- enum `DevUvcMode`: **no switch/string table in libdev** — value chosen by OBSBOT Center app layer; getter returns 1 byte. Values **UNVERIFIED-BY-EYES** (toggle UVC mode in Center and watch frame B8 10).
- Lite: same frame; serial word 0 → sender no-serial treatment (established rule).

### cameraGetUvcModeR @0x1800FE470 (ordinal 252, rva 0xFE470)
- Path **AA** GET, kind **0x0C**, ID word **0x100B9** → pair **`B9 10`**.
- Sends `FUN_180007490(dev, frame, reply_buf[6144])`; response: **1 byte at reply payload offset 0** → `*param_1` (unsigned char&).

### cameraSetRecordResolutionR @0x180030800 (ordinal 369, rva 0x30800)
- Path **AA**, kind **0x10**, ID word **0x1001A** → pair **`1A 10`**.
- Arg: `DevVideoResType` u32 LE at struct+0x1C. Pass-through, no switch/validation/product branch.
- enum `DevVideoResType`: **not resolvable from libdev** — no resolution string/table (string sweeps for 2160/3840/1080p/720p/4k/fps found no enum table). Domain is shared with the two exports below (same arg type). Actual 4K/1080p/fps id values **UNVERIFIED-BY-EYES**; `is4K` confirms `Obsbot_ty2lite` is treated as a 4K-capable product.
- **No `cameraGetRecordResolutionR` export exists** in this build (absent from export table) — current record resolution comes back inside device status/report frames, not a dedicated GET.

### Bonus same-domain (roadmap-4 adjacent)
- `cameraSetKcpPreviewResolutionR` @0x180030F60 (ord 341): AA kind 0x10, word **0x10023** → pair **`23 10`**, u32 DevVideoResType @+0x1C.
- `cameraSetNdiRtspResolutionR` @0x180031000 (ord 358): AA kind 0x10, word **0x10024** → pair **`24 10`**, u32 DevVideoResType @+0x1C.

### Frame rate
- **No fps/framerate export** in the whole export table (grep `Frame|Fps|Rate` → only Bitrate/Track matches). Frame rate is not a wire command in libdev: it is negotiated host-side via UVC capture formats (or implied by the DevVideoResType id). Path: **host-only**.
- Bitrate is adjacent but separate: `cameraSetMainVideoBitrateLevelR` @0x180030CB0 (rva 199856), `cameraGetMainVideoBitrateLevelR` @0x180030B60 (rva 199616) — DevVideoBitLevelType, not analyzed further here.

---

## Item 6 — AI tracking mode / target types

### aiSetTrackingModeR @0x18001AC20 (ordinal 125, rva 0x1AC20)
- Signature: `int Device::aiSetTrackingModeR(AiVerticalTrackType)`.
- Path **AA**, kind **0x0D**, ID word **0x30011** → pair **`11 30`**.
- Arg: **1 byte** `AiVerticalTrackType` at struct+0x1C. Pass-through, no switch, no product branch.
- enum `AiVerticalTrackType`: **unknown domain 0..N** (no table/strings in libdev). Values **UNVERIFIED-BY-EYES**.
- NOTE: an earlier note located "cameraSetTrackingTypeR @0x18002fd50" — **that is a mislabel for this build**: 0x18002FD50 resolves to `Device::cameraSetLedCtrlU` @0x18002FD20 (sel6 field 0x0118 LED toggle). There is **no camera\*TrackingType export** in this libdev. The live-confirmed "tracking type 1=Human" toggle therefore went through `cameraSetAiModeU` (sel6 0x0216, below) and/or `aiSetAiTrackModeEnabledR`.

### aiSetAiTrackModeEnabledR @0x180020CC0 (ordinal 63, rva 0x20CC0) — the target-type setter
- Signature: `int Device::aiSetAiTrackModeEnabledR(AiTrackModeType type, bool enable)`.
- Path **AA**, kind **0x14** (two u32 args), ID word **0x30071** → pair **`71 30`** (select target type) or **0x30072** → pair **`72 30`** when `type == 0x1E` (auto-group enable; args both zero).
- `enable == false`: no 71/72 frame — routes to `aiDelAutoGroupModeR` @0x1800207E0 (pair **`73 30`**, kind 0x0C, no args) if type==0x1E, else `aiDelSelectedTargetR` @0x180020750 (pair **`70 30`**, kind 0x0C, no args; its error string still says "aiSetSelectBiggestTarget" — leftover log text).
- **The full tracking_type switch** (enable==true, type != 0x1E) — args are u32 at +0x1C = catLo, u32 at +0x20 = subHi:

| AiTrackModeType id | wire catLo (+0x1C) | wire subHi (+0x20) | meaning |
|---|---|---|---|
| 1 | 0 | 0 | **Human** (live-confirmed) |
| 2 | 0 | 4 | UNVERIFIED-BY-EYES (target class 0, sub 4) |
| 3 | 0 | 3 | UNVERIFIED-BY-EYES (class 0, sub 3) |
| 4 | 0 | 2 | UNVERIFIED-BY-EYES (class 0, sub 2) |
| 5 | 0 | 1 | UNVERIFIED-BY-EYES (class 0, sub 1) |
| 6..9, others | — | — | default: **return -1, no frame sent** |
| 0xA (10) | 1 | 0 | UNVERIFIED-BY-EYES (class 1, sub 0 — candidate pet class) |
| 0xB | 1 | 2 | UNVERIFIED-BY-EYES (class 1, sub 2) |
| 0xC | 1 | 1 | UNVERIFIED-BY-EYES (class 1, sub 1) |
| 0x1E (30) | 0 | 0 | **auto-group** (pair 72 30) |

- **Naming**: string sweeps (`group|people|pet|single|multi|full body|half body|framing|auto.?group|track.?mode|aimode|work.?mode`) found **no enum-name strings** in libdev — only error-log function names. The two-wire-byte encoding (cat 0 vs 1, sub descending 0–4) is the only structural hint: ids 1–5 share category 0, ids A–C share category 1 (a second target-category family). Person/pet/group naming **must be settled by toggling the camera** (UNVERIFIED-BY-EYES).

### cameraSetAiModeU @0x18002FB10 (ordinal 275, rva 0x2FB10) — likely the Lite's live path
- Signature: `int Device::cameraSetAiModeU(AiWorkModeType workmode, int trackingtype)`.
- Path **sel6**: `FUN_1800087f0(dev, 6, buf)` 60-byte AiMode selector-6 write; **field id u16 at buf[0..1] = 0x0216**, buf[2] = workmode byte (param_1 low byte), buf[3] = param_2 low byte (tracking/target type), buf[4..59] zero.
- No product branch; this is the selector-6 path (same as MediaMode 0x0100 / WDR 0x0101 family).
- Enums: `AiWorkModeType` unknown domain; param_2 shares the AiTrackModeType domain (1=Human live-confirmed). **UNVERIFIED-BY-EYES** for all other ids.

### cameraSetBootModeU @0x18002FDD0 (ordinal 299, rva 0x2FDD0)
- Signature: `int Device::cameraSetBootModeU(AiWorkModeType, AiSubModeType)`.
- Path **sel6**, **field id 0x0119**, buf[2] byte = `(workmode << 4) | submode` (packed nibble pair), rest zero. Sets the mode the camera applies at boot.

### aiSetWorkModeR @0x18001ACC0 (ordinal 128, rva 0x1ACC0)
- Path **AA**, kind **0x0F** (3 payload bytes), ID word **0x30063** → pair **`63 30`**.
- Args: +0x1C = AiMainModeType byte, +0x1D = AiTrackSubModeType byte, +0x1E = 0.
- Enums: **unknown domains** (no switch/table); values app-chosen. UNVERIFIED-BY-EYES.

### aiSetTrackSpeedTypeR @0x180021220 (ordinal 124) — AA kind 0x0D, word 0x30068 → pair **`68 30`**, 1 byte `AiTrackSpeedType`. Unknown domain 0..N.
### aiSetHandTrackModeR @0x18001B570 (ordinal 99) — AA kind 0x0D, word 0x30049 → pair **`49 30`**, 1 byte `AiHandTrackType`. Unknown domain 0..N.

### Target-selection family (item-6 adjacent, same AA frame style, all sub-flag +0x1E=3)
- `aiSetSelectedTargetR` @0x1800212C0 (ord 120): kind **0x24**, word 0x30082 → pair **`82 30`**; copies 24-byte `DevTargetSelection` (4×u32 + u64) from caller struct into args @+0x1C..+0x33 (pos/size floats + id, layout not further resolved).
- `aiSetTargetZoomTypeR` @0x180021370 (ord 123): kind **0x14**, word 0x30083 → pair **`83 30`**; u32 `DevTargetZoomType` @+0x1C, u32 `DevCustomizedZoomType` @+0x20. Unknown domains.
- `aiSetTargetViewTypeR` @0x180021410 (ord 122): kind **0x10**, word 0x30084 → pair **`84 30`**; u32 `DevTargetViewType` @+0x1C. **This is the candidate full-body/half-body framing setter** (framing strings exist only in Center, not libdev) — values UNVERIFIED-BY-EYES.
- Related (not wire-detailed here): aiSetSelectBiggestTarget @0x180020430?/rva 132432, aiSetSelectCentralTarget (rva 132608), aiSetSelectTargetByPos (rva 131920), aiSetSelectTargetByBox (rva 131920+), aiSetNoTrack (rva 132096 → 0x180020400), aiSetZoneTrackState (0x1800200E0).

---

## Verification status
- All addresses/kind bytes/ID words/payload offsets: read directly from Ghidra pseudocode this session (REA), names from shipped PDB symbols.
- Wire pair hex: derived by the established `0xS_ID → [ID, S<<4]` mapping, consistent with previously live-captured frames.
- UNVERIFIED-BY-EYES: DevUvcMode values, DevVideoResType id→(w,h,fps) mapping, AiVerticalTrackType values, AiTrackModeType ids 2–5/0xA–0xC semantics (person/pet/other), AiWorkModeType/AiSubModeType/AiTrackSubModeType domains, DevTargetViewType/ZoomType domains, and whether OBSBOT Center drives Lite tracking through sel6 0x0216 vs AA 71 30 (both unbranched and available).
- Suggested egress test: watch for `71 30`/`72 30`/`63 30` (AA) vs selector-6 field `16 02` writes when changing tracking type in OBSBOT Center on the Lite.
