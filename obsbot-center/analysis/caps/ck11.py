#!/usr/bin/env python3
"""Bitwise CRC-16 full-keysearch: poly/reflection/init/xorout over 27 frames.
Early-abort on first mismatch to keep it fast."""
import json, sys
from concurrent.futures import ProcessPoolExecutor

xu = json.load(open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json'))
FR = []
for r in xu:
    for hexs in (r['out'], r['in']):
        if len(hexs) == 120 and hexs[:2] == 'aa':
            FR.append(bytes.fromhex(hexs))
FR = FR[:12]
N = len(FR)

def r8(x):
    return int('{:08b}'.format(x)[::-1], 2)

SPANS = [
    ('0:6+8:60', lambda b: b[0:6] + b[8:60]),
    ('full',     lambda b: b[0:60]),
    ('zck',      lambda b: b[0:6] + b'\0\0' + b[8:60]),
    ('8:60',     lambda b: b[8:60]),
]
SPAN_DATA = [(n, [sf(b) for b in FR]) for n, sf in SPANS]
WANT_BE = [int.from_bytes(b[6:8], 'big') for b in FR]
WANT_LE = [int.from_bytes(b[6:8], 'little') for b in FR]

def tables(poly, refin, init):
    """refin: input bytes reflected (LSB-first table-driven like crc16r)"""
    if refin:
        pr = int('{:016b}'.format(poly)[::-1], 2)
        t = []
        for i in range(256):
            c = i
            for _ in range(8):
                c = (c >> 1) ^ pr if c & 1 else c >> 1
            t.append(c)
        return t, True
    t = []
    for i in range(256):
        c = i << 8
        for _ in range(8):
            c = ((c << 1) ^ poly) & 0xFFFF if c & 0x8000 else (c << 1) & 0xFFFF
        t.append(c)
    return t, False

def run(t, buf, init, isref):
    c = init
    if isref:
        for by in buf:
            c = (c >> 8) ^ t[(c ^ by) & 0xFF]
    else:
        for by in buf:
            c = ((c << 8) & 0xFFFF) ^ t[((c >> 8) ^ by) & 0xFF]
    return c

def work(args):
    poly, out = []
    p0, p1 = args
    for poly in range(p0, p1):
        t0, ref0 = tables(poly, False, 0)
        t1, ref1 = tables(poly, True, 0)
        for sn, data in SPAN_DATA:
            for t, isref in ((t0, False), (t1, True)):
                for init in range(0x10000):
                    g0 = run(t, data[0], init, isref)
                    for xo in ((0, 0xFFFF) if (g0 ^ WANT_BE[0]) in (0, 0xFFFF) or (g0 ^ WANT_LE[0]) in (0, 0xFFFF) else ()):
                        # verify all frames, both byte orders, both xors
                        for order, WANT in ((WANT_BE,), (WANT_LE,)):
                            if all(run(t, d, init, isref) ^ 0 == w or run(t, d, init, isref) ^ 0xFFFF == w
                                   for d, w in zip(data, WANT)
                                   for _ in (0,)) if False else True:
                                pass
                        # simpler: check per order/xor combo
                        for order, WANT in ((WANT_BE,), (WANT_LE,)):
                            W = WANT
                            for xo in (0, 0xFFFF):
                                if all((run(t, d, init, isref) ^ xo) == w for d, w in zip(data, W)):
                                    out.append((hex(poly), sn, 'refl' if isref else 'norm', hex(init), hex(xo), 'BE' if W is WANT_BE else 'LE'))
    return out

if __name__ == '__main__':
    print('frames:', N, file=sys.stderr)
    CH = 2048
    chunks = [(i, i + CH) for i in range(0, 0x10000, CH)]
    hits = []
    with ProcessPoolExecutor(max_workers=16) as ex:
        for r in ex.map(work, chunks):
            hits.extend(r)
    print('HITS:', hits if hits else 'NONE')
