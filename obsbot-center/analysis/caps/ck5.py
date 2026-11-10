#!/usr/bin/env python3
"""CRC-table scan v2 + checksum brute (proper table verification)."""
import struct

DLL = '/mnt/c/Program Files/OBSBOT Center/bin/libdev.dll'
data = open(DLL, 'rb').read()

import json
xu = json.load(open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json'))
FRAMES = []
for r in xu:
    for hexs in (r['out'], r['in']):
        if len(hexs) == 120 and hexs[:2] == 'aa':
            FRAMES.append(bytes.fromhex(hexs))
FRAMES = FRAMES[:16]

spanfns = {'0:6+8:60': lambda b: b[0:6] + b[8:60], '8:60': lambda b: b[8:60],
           '0:60': lambda b: b[0:60], '10:60': lambda b: b[10:60],
           '9:60': lambda b: b[9:60], '12:60': lambda b: b[12:60],
           '0:2+8:60': lambda b: b[0:2] + b[8:60], '2:6+8:60': lambda b: b[2:6] + b[8:60]}
inits = [0x0000, 0xFFFF, 0x1D0F, 0x020A, 0x0A0D, 0x0C00, 0xAA01]
hits = []
for poly, kind, pos in tables:
    t = list(struct.unpack_from('<256H', data, pos))
    fn = crc_r if kind == 'refl' else crc_n
    for sname, sf in spanfns.items():
        for init in inits:
            for xorout in (0, 0xFFFF):
                for order in ('be', 'le'):
                    vals = []
                    for b in FRAMES:
                        got = fn(t, sf(b), init) ^ xorout
                        want = int.from_bytes(b[6:8], 'big' if order == 'be' else 'little')
                        vals.append(got == want)
                    if all(vals):
                        hits.append((hex(poly), kind, sname, hex(init), hex(xorout), order))
print('table-based hits:', hits if hits else 'NONE')
