# libdev.dll RE — Roadmap item 8 (gimbal presets), item 12 (yaw-dir reverse), factory reset

Binary: `C:\Program Files\OBSBOT Center\bin\libdev.dll` (doc `target-2486d1f3fe8b.dll`, base 0x180000000, RVA→VA = base+RVA; CSV RVAs are decimal).
All exports below go through the **0x10 / "AA" wire branch** (`FUN_180008180` enqueue at 0x180008180, frame builder `FUN_180100450` at 0x180100450). Product-0/1 (legacy) variants ride the V4L2 `FUN_1800087f0(dev, sel7, buf)` path — Lite (product 3) never does.

## Correction to the assumed command-struct model (verified this pass)

The staging struct passed to `FUN_180008180(dev, buf, param_3, param_4)` is **not** "kind at [0]". Verified layout (from builder + all setters):

| struct off | width | meaning |
|---|---|---|
| +0x00 | u16 | **total wire length** = 0x0c (header) + payload bytes (e.g. 0x0c GET-noargs, 0x0d GET/SET 1-byte arg, 0x10 4-byte arg, 0x20 trigger, 0x22/0x24 preset) |
| +0x04 | u32 | ack flag — **overwritten** by `FUN_180008180` to `(param_3&0xff)^1`; all exports pass 0 → builder emits wire flags 0x10 |
| +0x08 | u16 | **sequence — overwritten** with `++(dev+0x528 & 0xffff)` (auto) |
| +0x0a | u8 | ftype-source byte, = 3 for gimbal/preset cmds, = 1 for factory-reset cmds |
| +0x0c | u8 | fl-source byte, = 0x0b always (struct u16 0x0b00 at +0x0b) |
| +0x0e | u16 | **command ID** (0x07–0x44, see table) |
| +0x10 | u16 | **sub/serial-class S** — 3 for gimbal cmds, 1 for factory-reset cmds |
| +0x14 | u32 | serial word (`dev+0x1668`) — if == 3, builder switches to serial builder `FUN_1801006c0`; Lite serial word is 0 → plain builder `FUN_180100450` ("no-serial path") |
| +0x18 | u32 | dev_mode (`dev+0x3c`) — 0/2 forces transport gate: UVC channel `dev+0x3e0` must be open (else −7) and **len ≤ 0x3c** (else −5) |
| +0x1c | … | **args region**, copied verbatim to wire payload |

**Wire frame (FUN_180100450, no-serial):**
`[0]=0xAA | [1..2]=u16 len(&0xfff) | [3]=flags (0x10|ackbits; 0x10 for all these exports) | [4..5]=u16 seq | [6..8]=0 | [9]=nibble-remapped ftype/fl: (map_0x180218ef8-ish(+0x0c)<<4)|map(+0x0a) | [10..11]=u16 pair = (mapS(S)<<12)|(mapID(ID)&0xfff) | [0x0c..]=args from struct+0x1c`.
The three remaps are runtime-initialized `std::map`s (`DAT_180218f28/180218ef8/180218e28`) so exact nibbles are not statically resolvable; the **identity mapping** matches the working Rust port, so wire pair bytes = `[ID_low, S<<4]` (context notation `[ID, S<<4]`). No per-payload checksum/sub_ck exists on this branch (sub_ck belongs to the legacy type-0x21 branch); ack matching is done via pending-ack tables at `dev+0x4e8/0x4f8` with timestamp cleanup — the send itself is fire-and-forget (fails only if queue full ≥0xa00, UVC closed, or len>60 in dev_mode 0/2). "Kind 0x0C/0x0D/0x10" from earlier notes maps to struct **lengths** 0x0c/0x0d/0x10, not a wire kind byte.

Sync GETs (`aiGet*`, GetMethod != 1) use `FUN_180007490` (send + wait for response into a 6144-byte buffer; timeout internal to that helper, not decompiled here); GetMethod == 1 uses async `FUN_180007aa0` + callback.

## Command table (all verified from decompilation)

| export | VA | wire len | pair `[ID, S<<4]` | payload at wire+0x0c |
|---|---|---|---|---|
| aiGetGimbalPresetListR | 0x18001DFB0 | 0x0c | `35 30` | — (products 4/0xB/0x16/0x10/0x12/0x13 use `36 30`) |
| aiAddGimbalPresetR | 0x18001F380 | 0x24 (Lite) | `37 30` | 24-byte compact pos (below) |
| aiDelGimbalPresetR | 0x18001F860 | 0x10 | `38 30` | u32 preset id |
| aiUpdGimbalPresetR | 0x18001F9E0 | 0x22 (Lite, false) | `39 30` / `41 30` (param_2=true) | 18-byte compact pos + u32 id |
| aiSetGimbalPresetNameWithIdR | 0x18001F210 | 0x11+strlen | `3d 30` | u32 id, then name bytes + NUL |
| aiTrgGimbalPresetR (goto) | 0x18001FC20 | 0x20 | `3e 30` | u32 id + 4× f32 1.0f (scale/reserved) |
| aiGetGimbalBootPosR | 0x18001A530 | 0x0c | `08 30` | — (dev_mode 0/2 fallback adds 1 arg 0 → len 0x0d) |
| aiSetGimbalBootPosR | 0x18001A230 | 0x1c (serial-word 0) | `07 30` / `44 30` (param_2=true) | 4× f32 (struct +0x04,+0x08,+0x0c,+0x10) |
| aiGet... boot trig | 0x18001AA20 | 0x0c | `0a 30` | — (arg byte 1 only if serial word==3 && bool) |
| aiRstGimbalBootPosR | 0x18001AAD0 | 0x0c | `09 30` | — |
| aiSetGimbalYawDirReverseR | 0x180020220 | 0x0d | `65 30` (products 0/1/2/3/A/D/C) else `66 30` | 1 byte = bool |
| cameraSetRestoreFactorySettingsR | 0x18002E3E0 | 0x0d | `01 10` (S=1, ftype-src byte 1) | 1 byte = 0x01 |
| cameraSetFactorySettingsR | 0x1800FA950 | 0x0c | `01 10` | — (no arg) |

## Item 8 — preset management detail

**PresetPosInfo** (device struct, floats): `[0]`=preset id (copied as u32), `[1..6]` (+0x04..0x14) pose floats (yaw/pitch/roll/zoom + 2 more — axis order inferred, UNVERIFIED-BY-EYES), `[0x17..0x19]` (+0x5c/0x60/0x64) roll/pitch offset fields used only by compact packers.

**Add (Lite, dev_mode 0/2):** payload = `FUN_180018d20` compact 24B. Product 3 matches bitmask `0x0200340C` → **raw f32** layout: `[0]=id | +4 = struct+0x0c | +8 = +0x04 | +0x0c = +0x08 | +0x10 = +0x10 | +0x14 = +0x14`. Non-matching products instead get a 20B ×100-int16 encoding (yaw*100 @+8, pitch*100 @+6, +0x0c @+4, (zoom+0.001)*100 @+0x0a, 4× offset combos @+0x0c..0x13, u32 0 @+0x14). `FUN_180018d20` returns 0x14, code forces 0x18. Immediately after, add calls `FUN_1801888c0` (local preset-store insert) + `FUN_18017e480` (index fix-up) + `FUN_180189980` (persist) — no wire ack needed for the store.
**Long form** (dev_mode ∉ {0,2}): payload = first 0x24 bytes of PresetPosInfo + u32 id + u32 (presetIdx−1) + f[+0x5c] + f[+0x60] + (id<0x21 ? 0.5f : 0) + f[+0x64] + **name string + NUL**; total len = 0x32+strlen (stack-slot model is ambiguous about exact name offset, byte-exact needs capture — Lite path avoids it). Products 0/1 instead go V4L2 sel7 (`cameraAddGimbalPresetU`).
**Update vs add vs delete on the wire:** identical header; differ only by pair + payload — add `37 30` (no id-only arg; id inside 24B blob), delete `38 30` (u32 id only), update `39 30` (18B compact pos **plus trailing u32 id at wire+0x1e** — `FUN_1800185a0/600` packers), rename `3d 30` (u32 id + NUL name). Update with param_2=true uses pair `41 30` and a 24-byte pos pack (`FUN_1800184a0`) + id. Preset **id/index is the u32 at struct+0x1c (wire+0x0c)** for delete/update/rename; for add/goto it is the first u32 of the pos blob (wire+0x0c).
**Preset NAME embedding (rename, the path to use for Lite names):** wire+0x0c = u32 id, wire+0x10 = raw name bytes, terminated by 0x00; len = 0x11+strlen. Encoding: raw UTF-8 bytes unless `dev+0x17b8 == 1`, in which case `FUN_18017fca0` code-page converts first (UTF-8 → local ANSI). Max name ~47 bytes in dev_mode 0/2 (transport gate len ≤ 0x3c). **No trailing sub_ck on this branch** — the 043b/sub_ck struct is the legacy type-0x21 branch only.
**List GET:** response = u8 count followed by `count` u8 preset ids. **Goto:** pair `3e 30` + id + four 1.0f floats. The Rust port's working preset-goto uses the legacy 0x21 branch pair 0x043b; whether Lite firmware accepts the 0x10-branch `3e 30` goto is **UNVERIFIED-BY-EYES**.
`aiGetGimbalPresetNameWithIdR` (RVA 126512 → VA 0x18001EE30) exists per exports CSV (GET name-by-id, DevDataArray out) — not decompiled this pass.

## Item 12 — aiSetGimbalYawDirReverseR

VA 0x180020220. Payload = **single byte, plain bool 0/1** (no enum). Pair `65 30` on product 3 (Lite) — `66 30` only on other product types (gate: 0,1,2,3,0xA,0xD,0xC → 0x65). Struct len 0x0d, ftype-src 3, S=3. No productType-0xB/0xC/0xD/0x10/0x12/0x13/0x16 special handling beyond that list. Fire-and-forget; ack via pending-ack path.

## Factory reset — cameraSetRestoreFactorySettingsR

VA 0x18002E3E0 (RVA 189408). Frame: len **0x0d**, ftype-src byte 1 (differs from gimbal's 3 — remapped ftype nibble differs), fl-src 0x0b, **pair `01 10` (ID 0x01, S=1 — the only gimbal-adjacent export with S=1)**, **payload = exactly one byte: 0x01** (confirm arg; no other magic/confirm constant anywhere in the function). Companion `cameraSetFactorySettingsR` (0x1800FA950) sends the same `01 10` pair with **no payload byte at all** (len 0x0c) and the same ftype-src 1. No product-type branch: **Lite takes it on the no-serial path** (serial word 0 → builder `FUN_1800100450`). dev_mode 0/2 gate: transport requires UVC channel open and len 0x0d ≤ 0x3c — always satisfied. **No client-side timeout**: `FUN_180008180` only enqueues (returns −3 queue-full / −7 UVC-closed / −5 too-long synchronously); success of the reset itself is observable only via device response/disconnect.
