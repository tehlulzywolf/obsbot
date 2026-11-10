#!/usr/bin/env python3
"""Cross-check captured frames vs Rust codec rules:
1. Is wire-cmd at [10..11] == 0x0FFF (local USB set-0 sentinel) or a real wirecode?
2. Is byte[3] one of 0x12/0x10/0x11?
3. Does the version byte [9] fit nibble-map pattern (0x0D/0x02/0x04/0x12 observed)?
4. Compare against captured 12B-header frames with nonzero payload past 12:
   print payload region [12..] fully for the interesting cmds.
Also decode cmd words through SETCODE/INNER maps to see which app cmds they are."""
import json

recs = json.load(open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/ctrl9.json'))
frames = []
for r in recs:
    p = bytes.fromhex(r['payload']) if r['payload'] else b''
    if len(p) >= 12 and p[0] == 0xaa:
        frames.append(p)

INNER = {
 1: {0:461,1:964,2:962,3:978,4:466},
 2: {0:1,2:4,3:6,4:10,5:34,6:35},
 3: {0:35,7:53,8:54,10:58,13:81,14:102,15:103,16:144,17:145,19:146,21:148,91:147,101:238},
 5: {0:1,3:16,4:96,5:97,6:100,7:101,8:99,9:102,10:32,11:105,12:106,13:107},
}
SETCODE = {0:0,1:1,2:2,3:3,4:4,5:8,6:13}
INV_SETCODE = {v: k for k, v in SETCODE.items()}
INV_INNER = {sc: {v: k for k, v in m.items()} for sc, m in
             zip([1,2,3,5], [INNER[1], INNER[2], INNER[3], INNER[5]])}

print(f"{'b1':>3} {'b2':>3} {'b3':>3} {'b4-5':>5} {'b6-7':>5} {'b8':>3} {'b9':>3} {'cmd':>5}  decode")
for p in frames:
    wcmd = p[10] << 8 | p[11]
    setcode = wcmd >> 12
    wc = wcmd & 0xFFF
    cs = INV_SETCODE.get(setcode)
    dec = ''
    if wcmd == 0x0FFF: dec = 'set0 sentinel'
    elif cs is not None and wc in INV_INNER.get(setcode if setcode in (1,2,3,5) and setcode or -1, {}):
        dec = f'set{cs} cmd{INV_INNER[setcode][wc]}'
    else:
        dec = f'setcode{setcode} wire{wc}'
    print(f"{p[1]:3d} {p[2]:3d} {p[3]:3d} {p[4]:02x}{p[5]:02x} {p[6]:02x}{p[7]:02x} {p[8]:3d} {p[9]:02x} {wcmd:#06x}  {dec}")

print("\n=== payloads [12..40] for frames w/ nonzero beyond 12 ===")
for p in frames:
    nz = [i for i in range(12, len(p)) if p[i]]
    if nz:
        end = max(nz) + 1
        print(f"cmd={p[10]:02x}{p[11]:02x} len={end}: {p[12:end].hex()}")
