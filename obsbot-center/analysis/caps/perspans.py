#!/usr/bin/env python3
"""Per-frame: for each captured AA frame, find ALL spans 1..1024 (zero-padded 4K buf,
zero67 / zero678) where the split-table CRC reproduces wire bytes 6,7. Intersect."""
import json, struct as st
data = open('/mnt/c/Program Files/OBSBOT Center/bin/libdev.dll', 'rb').read()
e = st.unpack_from('<I', data, 0x3C)[0]
nsec = st.unpack_from('<H', data, e + 6)[0]
optsz = st.unpack_from('<H', data, e + 20)[0]
secs = []
for i in range(nsec):
    b = e + 24 + optsz + i * 40
    secs.append(st.unpack_from('<IIII', data, b + 12))
def va2off(va):
    rva = va - 0x180000000
    for sva, svz, sraw, spr in secs:
        if sva <= rva < sva + max(svz, spr):
            return sraw + (rva - sva)
TA = list(data[va2off(0x1801cca60):va2off(0x1801cca60) + 256])
TB = list(data[va2off(0x1801ccb60):va2off(0x1801ccb60) + 256])
xu = json.load(open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json'))
reqs, resps = [], []
for r in xu:
    if len(r['out']) == 120 and r['out'][:2] == 'aa':
        reqs.append(bytes.fromhex(r['out']))
    if len(r['in']) == 120 and r['in'][:2] == 'aa':
        resps.append(bytes.fromhex(r['in']))

def hits_for(w):
    out = set()
    for z8 in (0, 1):
        buf = bytearray(4096); buf[0:60] = w
        buf[6] = buf[7] = 0
        if z8: buf[8] = 0
        b4 = b6 = 0xFF
        want = (w[6], w[7])
        for i in range(1, 1025):
            idx = b6 ^ buf[i - 1]; b6 = b4 ^ TA[idx]; b4 = TB[idx]
            v = ~((b4 << 8) | b6) & 0xFFFF
            if ((v >> 8) & 0xFF, v & 0xFF) == want:
                out.add((i, z8))
    return out

common = None
for i, w in enumerate(reqs):
    h = hits_for(w)
    common = h if common is None else (common & h)
    print('req', i, w[:12].hex(' '), '->', (sorted(h)[:6] or 'NONE'))
print('COMMON reqs:', sorted(common)[:12] or 'NONE')
cr = None
for w in resps:
    h = hits_for(w)
    cr = h if cr is None else (cr & h)
print('COMMON resps:', sorted(cr)[:12] or 'NONE')
