#!/usr/bin/env python3
"""FUN_180163670: per-nibble CRC-16/CCITT (4x64 interleaved tables at 0x1801cca60).
Test every span x zero-mask combo on captured frames."""
import json
data = open('/mnt/c/Program Files/OBSBOT Center/bin/libdev.dll', 'rb').read()
blob = data[0x1cb060:0x1cb160]          # VA 0x1801cca60
T = [list(blob[0:64]), list(blob[64:128]), list(blob[128:192]), list(blob[192:256])]
# sanity: T0[1] should be 0x01 if poly 0x1021 MSB-first; check
def build1021():
    t = []
    for i in range(64):
        c = i << 8
        for _ in range(8):
            c = ((c << 1) ^ 0x1021) & 0xFFFF if c & 0x8000 else (c << 1) & 0xFFFF
        t.append(c & 0xFF)  # store low byte per decompile (bVar6 = bVar4 ^ T[..])
    return t
# use the SHIPPED tables directly (authoritative):
def crc_nib(buf, init=0xFFFF):
    lo = hi = init & 0xFF
    khi = (init >> 8) & 0xFF
    lo = init & 0xFF
    for b in buf:
        nib = b >> 4
        idx = (khi << 2) | (lo >> 6)
        khi = lo ^ T[3][idx]
        lo = ((lo << 2) & 0xFF) ^ T[2][idx] ^ nib
        nib = b & 0xF
        idx = (khi << 2) | (lo >> 6)
        khi = lo ^ T[1][idx]
        lo = ((lo << 2) & 0xFF) ^ T[0][idx] ^ nib
    return ((khi << 8) | lo) & 0xFFFF

xu = json.load(open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json'))
reqs, resps = [], []
for r in xu:
    if len(r['out']) == 120 and r['out'][:2] == 'aa':
        reqs.append(bytes.fromhex(r['out']))
    if len(r['in']) == 120 and r['in'][:2] == 'aa':
        resps.append(bytes.fromhex(r['in']))

def test(tag, frames):
    best = []
    for span in [('1:L', 1, None), ('0:L', 0, None), ('10:end', 10, 60), ('2:end', 2, 60)]:
        for zpos in [(), (6, 7), (6, 7, 8), (8,), (6, 7, 8, 9)]:
            for Lsrc in ['b1', 'b2b1&fff', 'L12']:
                ok = 0
                for b in frames:
                    if Lsrc == 'b1':
                        L = b[1] & 0xFF
                    elif Lsrc == 'b2b1&fff':
                        L = ((b[1] << 8) | b[2]) & 0xFFF
                    else:
                        L = 12
                    if L == 0 or L > 60:
                        continue
                    buf = bytearray(b)
                    for p in zpos:
                        buf[p] = 0
                    a, z = span[1], span[2] or L
                    z = min(z, L)
                    v = crc_nib(buf[a:z])
                    want_be = int.from_bytes(b[6:8], 'big')
                    want_le = int.from_bytes(b[6:8], 'little')
                    if v == want_be or v == want_le or (~v & 0xFFFF) == want_be or (~v & 0xFFFF) == want_le:
                        ok += 1
                best.append((span[0], zpos, Lsrc, ok, len(frames)))
    best.sort(key=lambda x: -x[3])
    print(tag, best[:5])

test('REQ ', reqs)
test('RESP', resps)
