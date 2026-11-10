#!/usr/bin/env python3
"""Checksum hunt v2 — using ground-truth pairs from tx9g.
Known 60B bodies. Request has ck at [6:8]; find algo over spans.
Pairs (same seq): f1496req/f1506resp seq0, f1510/f1516 seq1,
f1520/f1524 seq2, f1532/f1536 seq3, f2220req/f2226resp seq4(cmd42c4)."""
import itertools, json

REQ = {
 0: 'aa0100000c00915c0a0d0818000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000',
 1: 'aa0101000c00c1500a0d080400000000000037be8bbcbd72000000000000000000000000000000000000000000000000000000000000000000000000',
 2: 'aa0102000c00c0960a0d4819000000000000000000000000000037be8bbcbd7201000000000000000000000000000000000000000000000000000000',
 3: 'aa0103000c0031530a0dc818000000000000000000000000000037be8bbcbd7201000000000000000000000000000000000000000000000000000000',
 4: 'aa0104000c00d6730a0242c4000000007000000000000000000027cb5c02000007000000000000000000000019000200600000000000000044210dc0',
 5: 'aa0105000c0095840a040401000000000000000000000000000037be8bbcbd7201000000000000000000000000000000000000000000000000000000',
}
RESP = {
 0: 'aa2900000c00a0490d0a0818180059fc0000000000000000000000000000000000000000000037be8bbcbd7200000000000000000000000000000000',
 1: 'aa2901000c00f0450d0a08040800540c010602060000000000000000000000000000000000000000000037be8bbcbd72010000000000000000000000',
 2: 'aa2902000c00f1830d0a48192200987f080008020106020600000000000000000000000000000000000037be8bbcbd72010000000000000000000000',
 3: 'aa2903000c0000460d0ac8180e0052f1524d4f574c4847423038314a494c000000000000000000000000000000000000000000000000000000000000',
 4: 'aa890400100046b7020a42c40002fdff503302020106020600000000000000000000000000000000000037be8bbcbd72010000000000000000000000',
}

def crc16(data, poly, init, refin, refout, xorout):
    def rev(x, n):
        r = 0
        for i in range(n):
            if x >> i & 1: r |= 1 << (n - 1 - i)
        return r
    crc = init
    for b in data:
        if refin: b = rev(b, 8)
        crc ^= b << 8
        for _ in range(8):
            crc = ((crc << 1) ^ poly) & 0xFFFF if crc & 0x8000 else (crc << 1) & 0xFFFF
    if refout: crc = rev(crc, 16)
    return crc ^ xorout

frames = []
for s, hexs in list(REQ.items()) + list(RESP.items()):
    frames.append(bytes.fromhex(hexs))

cands = []
for poly, init, xorout in [(0x8005, 0, 0), (0x8005, 0xFFFF, 0), (0x8005, 0xFFFF, 0xFFFF),
                           (0x1021, 0, 0), (0x1021, 0xFFFF, 0), (0x1021, 0, 0xFFFF),
                           (0x1021, 0x1D0F, 0),
                           (0xA001, 0, 0), (0x3D65, 0, 1)]:
    for refin in (False, True):
        for refout in (False, True):
            cands.append((poly, init, refin, refout, xorout))
cands = list(dict.fromkeys(cands))

spans = [('0:6+8:60', lambda b: b[0:6] + b[8:60]),
         ('0:6+8:L',   lambda b: b[0:6] + b[8:60]),
         ('2:6+8:60',  lambda b: b[2:6] + b[8:60]),
         ('8:60',      lambda b: b[8:60]),
         ('0:60',      lambda b: b[0:60]),
         ('2:60',      lambda b: b[2:60]),
         ('10:60',     lambda b: b[10:60]),
         ('0:2+8:60',  lambda b: b[0:2] + b[8:60]),
         ('6:60+0:6',  lambda b: b[6:60] + b[0:6]),
         ]
found = []
for sname, sf in spans:
    for poly, init, refin, refout, xorout in cands:
        ok_be = ok_le = True
        for b in frames:
            want = b[6:8]
            got = crc16(sf(b), poly, init, refin, refout, xorout)
            if got.to_bytes(2, 'big') != want: ok_be = False
            if got.to_bytes(2, 'little') != want: ok_le = False
            if not ok_be and not ok_le: break
        if ok_be or ok_le:
            found.append((sname, hex(poly), hex(init), refin, refout, hex(xorout), 'BE' if ok_be else 'LE'))
print('matches:', found if found else 'NONE')
# also: maybe ck bytes cover a length-truncated frame: real len = 12 + args_len, args_len unknown;
# but padded 60 — assume full 60 tested above. Try simple additive/xor folds:
for sname, sf in spans:
    for kind, fn in [('sum16', lambda bs: (sum(bs) & 0xFFFF)),
                     ('negsum', lambda bs: (-sum(bs)) & 0xFFFF),
                     ('xor16', lambda bs: __import__('functools').reduce(lambda a, b: a ^ b, [bs[i] << 8 | bs[i+1] for i in range(0, len(bs) - 1, 2)], 0))]:
        ok = all(fn(sf(b)).to_bytes(2, 'big') == b[6:8] or fn(sf(b)).to_bytes(2, 'little') == b[6:8] for b in frames)
        if ok: print('fold match:', sname, kind)
