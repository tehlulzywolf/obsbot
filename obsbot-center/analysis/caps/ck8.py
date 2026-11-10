#!/usr/bin/env python3
"""Checksum = crc8(T1) . crc8(T2)? DLL tables at 0x1cb060/0x1cb160 (256B each, CRC-8).
Also try single CRC-8s from any 256B tables in the DLL, over many spans,
comparing against both ck bytes individually AND the u16 pair."""
import struct, json

DLL = '/mnt/c/Program Files/OBSBOT Center/bin/libdev.dll'
data = open(DLL, 'rb').read()
xu = json.load(open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json'))
FRAMES = []
for r in xu:
    for hexs in (r['out'], r['in']):
        if len(hexs) == 120 and hexs[:2] == 'aa':
            FRAMES.append(bytes.fromhex(hexs))

def crc8(t, buf, init=0, xorout=0):
    c = init
    for by in buf:
        c = t[c ^ by]
    return c ^ xorout

# find all plausible 256B CRC-8 tables (t[0]==0, t[1] is a byte-poly)
tabs = []
for off in range(0, len(data) - 256, 16):
    if data[off] != 0: continue
    p = data[off + 1]
    if p == 0: continue
    # verify non-reflected poly p
    ok = True
    for i in (1, 2, 0x80, 0xFF):
        c = i
        for _ in range(8):
            c = ((c << 1) ^ p) & 0xFF if c & 0x80 else (c << 1) & 0xFF
        if c != data[off + i]: ok = False; break
    if ok: tabs.append((p, 'n', off)); continue
    pr = int('{:08b}'.format(p)[::-1], 2)
    ok = True
    for i in (1, 2, 0x80, 0xFF):
        c = i
        for _ in range(8):
            c = (c >> 1) ^ pr if c & 1 else c >> 1
        if c != data[off + i]: ok = False; break
    if ok: tabs.append((p, 'r', off))
uniq = {}
for p, m, o in tabs:
    uniq.setdefault((p, m), o)
print('CRC-8 tables:', [(hex(p), m, hex(o)) for (p, m), o in uniq.items()])

spans = {
 '0:6+8:60': lambda b: b[0:6] + b[8:60], '8:60': lambda b: b[8:60],
 '0:60': lambda b: b[0:60], '0:60z': lambda b: b[0:6] + b'\0\0' + b[8:60],
 '10:60': lambda b: b[10:60], '9:60': lambda b: b[9:60], '12:60': lambda b: b[12:60],
 '0:2+8:60': lambda b: b[0:2] + b[8:60], '2:6+8:60': lambda b: b[2:6] + b[8:60],
 '0:6': lambda b: b[0:6], '0:12z': lambda b: b[0:6] + b'\0\0' + b[8:12],
}
inits = [0, 0xFF]
xors = [0, 0xFF]
items = [(pp, mm, oo) for (pp, mm), oo in uniq.items()]
hits = []
# pattern A: ck = (crc8_t1 << 8) | crc8_t2  (and swap)
for (p1, m1, o1) in items:
    tt1 = data[o1:o1 + 256]
    for (p2, m2, o2) in items:
        tt2 = data[o2:o2 + 256]
        for sname, sf in spans.items():
            for i1 in inits:
                for i2 in inits:
                    for x1 in xors:
                        for x2 in xors:
                            okAB = True; okBA = True
                            for b in FRAMES:
                                a = crc8(tt1, sf(b), i1, x1)
                                c = crc8(tt2, sf(b), i2, x2)
                                if (a << 8 | c) != int.from_bytes(b[6:8], 'big'): okAB = False
                                if (c << 8 | a) != int.from_bytes(b[6:8], 'big'): okBA = False
                                if not okAB and not okBA: break
                            if okAB: hits.append(('AB', hex(p1), m1, hex(p2), m2, sname, hex(i1), hex(i2), hex(x1), hex(x2)))
                            if okBA: hits.append(('BA', hex(p1), m1, hex(p2), m2, sname, hex(i1), hex(i2), hex(x1), hex(x2)))
        if len(FRAMES) > 4 and hits: print('EARLY HIT:', hits[-1]); break
print('CRC8-pair hits:', hits[:20] if hits else 'NONE')
# pattern B: ck bytes = two different CRC-8s over DIFFERENT spans (odd/even split?)
odd = lambda b: bytes(b[i] for i in range(0, 60, 2))
even = lambda b: bytes(b[i] for i in range(1, 60, 2))
spans2 = dict(spans); spans2['odd'] = odd; spans2['even'] = even
hits2 = []
for (p1, m1, o1) in items:
    tt1 = data[o1:o1 + 256]
    for sa, fa in spans2.items():
        for sb, fb in spans2.items():
            for i1 in inits:
                for i2 in inits:
                    if all(crc8(tt1, fa(b), i1) == b[6] and crc8(tt1, fb(b), i2) == b[7] for b in FRAMES):
                        hits2.append((hex(p1), m1, sa, sb, hex(i1), hex(i2)))
print('odd/even hits:', hits2[:10] if hits2 else 'NONE')
