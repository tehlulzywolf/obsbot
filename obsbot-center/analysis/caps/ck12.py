#!/usr/bin/env python3
"""All 2^16 polys, bitwise CRC, 4 spans x 2 reflections x 4 (init,xor) combos,
checked first against one frame => survivor set tiny => verify vs all.
Parallel over 16 workers."""
import json, sys
from concurrent.futures import ProcessPoolExecutor

xu = json.load(open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json'))
FR = []
for r in xu:
    for hexs in (r['out'], r['in']):
        if len(hexs) == 120 and hexs[:2] == 'aa':
            FR.append(bytes.fromhex(hexs))
FR = FR[:14]
COMBOS = [('0:6+8:60', lambda b: b[0:6] + b[8:60]), ('full', lambda b: b[0:60]),
          ('zck', lambda b: b[0:6] + b'\0\0' + b[8:60]), ('8:60', lambda b: b[8:60])]
DATA = [(n, [sf(b) for b in FR]) for n, sf in COMBOS]
WB = [int.from_bytes(b[6:8], 'big') for b in FR]
WL = [int.from_bytes(b[6:8], 'little') for b in FR]

def rb(x, bits):
    r = 0
    for i in range(bits):
        if x >> i & 1: r |= 1 << (bits - 1 - i)
    return r

def work(rng):
    a, z = rng
    hits = []
    for poly in range(a, z):
        pr = rb(poly, 16)
        for sn, data in DATA:
            b0 = data[0]
            # normal MSB-first
            for init, xo in ((0, 0), (0xFFFF, 0xFFFF), (0xFFFF, 0), (0, 0xFFFF)):
                c = init
                for by in b0:
                    c ^= by << 8
                    for _ in range(8):
                        c = ((c << 1) ^ poly) & 0xFFFF if c & 0x8000 else (c << 1) & 0xFFFF
                if (c ^ xo) == WB[0] or (c ^ xo) == WL[0]:
                    for W in (WB, WL):
                        ok = True
                        for d, w in zip(data[1:], W[1:]):
                            c = init
                            for by in d:
                                c ^= by << 8
                                for _ in range(8):
                                    c = ((c << 1) ^ poly) & 0xFFFF if c & 0x8000 else (c << 1) & 0xFFFF
                            if (c ^ xo) != w: ok = False; break
                        if ok:
                            hits.append((hex(poly), sn, 'n', hex(init), hex(xo), 'BE' if W is WB else 'LE'))
                # LSB-first (reflected, poly also reflected internally)
                c = init
                for by in b0:
                    c ^= by
                    for _ in range(8):
                        c = (c >> 1) ^ pr if c & 1 else c >> 1
                if (c ^ xo) == WB[0] or (c ^ xo) == WL[0]:
                    for W in (WB, WL):
                        ok = True
                        for d, w in zip(data[1:], W[1:]):
                            c = init
                            for by in d:
                                c ^= by
                                for _ in range(8):
                                    c = (c >> 1) ^ pr if c & 1 else c >> 1
                            if (c ^ xo) != w: ok = False; break
                        if ok:
                            hits.append((hex(poly), sn, 'r', hex(init), hex(xo), 'BE' if W is WB else 'LE'))
    return hits

if __name__ == '__main__':
    print('frames:', len(FR), file=sys.stderr)
    CH = 1024
    with ProcessPoolExecutor(max_workers=16) as ex:
        res = list(ex.map(work, [(i, i + CH) for i in range(0, 0x10000, CH)]))
    hits = [h for r in res for h in r]
    print('HITS:', hits if hits else 'NONE')
