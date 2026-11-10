# Roadmap items 1, 2, 3, 9 — libdev.dll RE findings (2026-11-09)

Binary: `C:\Program Files\OBSBOT Center\bin\libdev.dll` (PDB GUID 2ca512b4-4286-3796-eac9-e73cb000b3c2), Ghidra 12.1.4 via REA.

## Internal-word → wire-pair rule (re-confirmed this session)
Decompiler setter pattern `local_1828[0] = kind; local_181a = 0xSIDD` maps to our
frame codec as: kind = wire byte1 family (0x0C GET, 0x0D SET_SIMPLE, 0x10 SET_ACK, 0x14 AI/ratio),
internal word `0xS_ID` → **wire pair bytes10-11 = [ID, S<<4]** (e.g. `0x100A0` → `[A0,10]`,
matches tally/boot captures). Extra struct field `0xb00` at +0xb is constant in every setter.

Two device-path switches in libdev:
- `dev+0x3c` "dev_mode": 0 = AA/AiMode protocol (our Tiny 2 Lite — confirmed: every AA
  GET/SET we send is acked, and AA-path getters work; dev_mode 1/2 = old models that
  read a host-side cache instead).
- serial word `dev+0x1668`: 0 = no-serial path → setter kind becomes **0x0D SET_SIMPLE**
  (sleep/wake already live-confirmed on this no-serial path, so the Lite has serial word 0
  in the field libdev checks).
- productType exclusion sets `{4,0xb,0x16,0x10}` (WDR, zoom) and `{0xb,0x16,0x10}` (zoom set):
  force the AA pair even when dev_mode==0. Live tests below settle which way the Lite goes.

## AiMode selector-6 struct (reused by many controls)
`FUN_1800087f0(dev, 6, buf)` sends XU **selector 6** SET_CUR with a 60-byte struct:
bytes 0-1 = u16 LE field ID, value bytes from offset 2, rest zero.
Known field IDs from decompile: `0x0100` MediaMode (1 byte), `0x0101` WDR/HDR (byte, param!=0),
`0x0103` FaceAE (byte), `0x1021` FieldView (4×u32, 4-byte aligned).
GET_CUR sel6 returns the current blob; patch one field, SET back (pattern proven in
`examples/probe_aimode.rs`, but that sweep watched only gimbal attitude — wrong observation target).

---

## Item 1 — Face focus + face exposure

| Export | Addr | Internal | Wire pair | Kind | Args |
|---|---|---|---|---|---|
| cameraSetFaceFocusR | 0x18002f6e0 | 0x10003 | **03 10** | 0x0D (serial word 0 → no-serial path, same as sleep/wake) | u32 bool 0/1 |
| cameraSetAutoFocusModeR | 0x1800f69b0 | 0x1005f | **5F 10** | 0x10 SET_ACK | u32 DevAutoFocusType |
| cameraGetAutoFocusModeR | 0x1800f6a50 | 0x1005e | **5E 10** | 0x0C GET | — (reply payload u32 = mode) |
| cameraSetFaceAER | 0x18002eab0 | 0x10045 | **45 10** (dev_mode 1/2 path) — **Lite path = AiMode sel6 field 0x0103 byte** | see note | byte/u32 0/1 |
| cameraGetFaceAER | 0x1800f39b0 | 0x10044 | **44 10** (dev_mode 1/2) | 0x0C | — |
| cameraSetExposureModeR | 0x1800f3aa0 | 0x10047 | **47 10** | 0x10 SET_ACK | u32 mode |
| cameraGetExposureModeR | 0x1800f3b40 | 0x10046 | **46 10** | 0x0C GET | — (reply u32 = mode) |

- Face focus is a **separate vendor bool riding on top of** the UVC auto-focus switch:
  UVC `auto_focus` (CID 0x9E05) untouched by these; Global/Face = DevAutoFocusType enum
  via 5F 10, plus the 03 10 presence flag. FaceAE sits on AiMode sel6 (Lite) so the
  earlier "byte sweep silent" result doesn't disprove it — the sweep never watched a
  face-exposure reaction.
- DevAutoFocusType integer values are chosen by the caller (OBSBOT_Main), not in libdev:
  settle by live sweep + 5E 10 readback (device echoes accepted value).
- Exposure mode values likewise settle via 47 10 sweep + 46 10 readback.

## Item 2 — HDR / WDR

| Export | Addr | Path | Wire |
|---|---|---|---|
| cameraSetWdrR | 0x18002f180 | dev_mode==0 AND product ∉ {4,0xb,0x16,0x10} | **AiMode sel6 field 0x0101**, byte = (param != 0) |
| | | else | internal 0x10014 → pair **14 10**, kind 0x10 SET_ACK, arg u32 value |

- Branch depends on the Lite's productType (not yet extracted); test **both** on device:
  visual HDR on/off in our own preview is the verification. 0 = WDR off, 1 = WDR on
  (decompile passes raw param to AA; sel6 path stores bool).
- No WDR getter exists in libdev (searched "GetWdr" family) → readback is visual only.

## Item 3 — Device mirror / flip / rotate

| Export | Addr | Internal | Wire pair | Kind | Args |
|---|---|---|---|---|---|
| cameraSetMirrorFlipR | 0x180031600 | 0x1002e | **2E 10** | 0x10 SET_ACK | u32 value |
| cameraGetMirrorFlipR | 0x1800316a0 | 0x1002d | **2D 10** | 0x0C GET | — (dev_mode 1/2 → cache +0xc44; Lite → device GET, reply u32) |
| cameraSetMediaModeU | 0x18002f790 | MediaMode enum | **AiMode sel6 field 0x0100**, byte = enum | — | portrait 9:16 candidate |
| cameraSetFieldView | 0x1800f89c0 | — | **AiMode sel6 field 0x1021** + 4×u32 (x,y,w,h likely) | — | digital-FOV box candidate |

- MirrorFlip = single int combining mirror/flip; sweep 0..3 with 2D 10 readback +
  visual check (sensor-side flip visibly mirrors our MJPEG preview).
- 90° rotation: candidates are MediaMode (portrait) or FieldView; no dedicated
  "rotate90" export in libdev (searched Rotate/Rotation — cameraGetRotate etc. checked
  during sweep; record result after probe run).

## Item 9 — Real zoom readout (RESOLVED, no vendor command needed)

- cameraSetZoomAbsoluteR (0x180029ba0): Lite path (dev_mode==0, product ∉ {0xb,0x16,0x10}) =
  scale ratio → UVC range and **write the UVC zoom-absolute control** ("CameraControl_Zoom",
  am_cam_ctrl vtable +0x20, arg = (ratio-1)*(max-min) scaled, speed=2).
  → Our `zoom_absolute 0-100` send is exactly what Center does. ✔ keep.
- cameraGetZoomAbsoluteR (0x180029e10): Lite path = UVC GET_CUR zoom (vtable +0x28),
  then **ratio = 1 + (raw − min) / (max − min)** (products 0xC/0xD use raw*0.25+1).
- cameraGetRangeZoomAbsoluteR (0x1800299a0): Lite path = UVC probe GET_LEN/MIN/MAX/RES
  (vtable +0x18, cached) → **V4L2 QUERYCTRL min/max/step/def** is the same data.
- normalizedZoom (0x180011b90): raw→0-100 percent via the range. Our slider is this scale.
- aiSetCameraZoomRatioR (0x18001dec0): separate AA cmd, internal 0x30034 (AI set, id 0x34),
  kind **0x14**, pair **34 30**, args f32 ratio + i32 — used for AI auto-zoom, NOT the
  manual slider on this model.

**Panel fix:** `VIDIOC_QUERYCTRL(CID_ZOOM_ABSOLUTE)` once for min/max, poll `G_CTRL`
for raw, label = `1 + (raw−min)/(max−min)` with 2 decimals. Replaces the `1+v/100*3` guess.

---

## Live probe results (probe_items, 2026-11-09, serial RMOWLHGB081JIL)

- ZOOM ctrl QUERYCTRL: **min=0 max=100 step=1 def=0**, live G_CTRL cur=27 → real
  readback works.

## Zoom, fully resolved (cameraGetZoomAbsoluteR 0x180029e10, cameraSetZoomAbsoluteR
0x180029ba0, cameraGetRangeZoomAbsoluteR 0x1800299a0)

libdev's zoom stack for dev_mode 0 (Lite) is a V4L2 shim (`am_cam_ctrl`, vtable):
- get_range(3) = **VIDIOC_QUERYCTRL**(0x9A0201 | 3<<16 = CID_ZOOM_ABSOLUTE) → min/max/step/def
- get(3) = **VIDIOC_G_CTRL**(CID_ZOOM_ABSOLUTE) → cur
- set(3, v, flag=2) = **VIDIOC_S_CTRL**(CID_ZOOM_ABSOLUTE, v) — exactly what the panel does
- ratio display: product-dependent:
  - products 0xC/0xD: ratio = cur*0.25 + 1
  - everything else (incl. Lite): ratio = **1 + (cur−min)/(max−min)** → with our
    0..100 range that is 1.00–2.00; Center shows the same, so the panel label now
    uses this formula verbatim (was 1+3·pct).
- set path for Lite: raw = (max−min)*(ratio−1) + min, S_CTRL. AA-pair zoom exists
  only for products {0xB, 0x16, 0x10, 0xC, 0xD}; NOT the Lite.

## ObsbotProductType enum (from FUN_180028820 switch, dev+0x680)

```text
2   = Tiny2          3 = Tail2Lite  (= OBSBOT Tiny 2 Lite, our device, VID_3564 PID_FEF9)
4   = TailAir       0xB = Tail2
0xC = TinySE        0xD = (unnamed in this switch; zoom 0.25 branch partner)
0x10= Tail2s        0x12 = Tiny3    0x13 = Tiny3Lite
```
Device-name strings at 0x1801ce0a8..: Obsbot_tiny_2P / Obsbot_tiny2 / Obsbot_ty2lite /
Obsbot_tinyse / Obsbot_tiny3 / Obsbot_tiny3lite. dev+0x680 holds the product type.
- SETs device-ACKed: face_focus `03 10` (on+off), WDR AA `14 10`, mirror `2E 10`,
  AF `5F 10`, exp `47 10` all accepted (status ACK, same shape as sleep/wake).
  AiMode sel6 `0101` (HDR) and `0103` (FaceAE) XU SETs succeeded (no ACK concept on XU).
- GETs `5E 10`, `46 10`, `2D 10` all **silent** (no reply) — Tiny 2 Lite firmware
  doesn't implement set-1 readbacks for these, same class as tally GET. Enum
  semantics therefore settle by watching the preview, not by echo readback.
