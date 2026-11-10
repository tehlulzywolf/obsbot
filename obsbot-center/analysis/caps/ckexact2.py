#!/usr/bin/env python3
"""Corrected: loop span = FRAME LENGTH (60), uVar3 low nibble=type, bits 0..11 carry
seq-mixed value 0x100|seq. Test L variants + byte8 keep/zero + store order."""
import json, struct
data = open('/mnt/c/Program Files/OBSBOT Center/bin/libdev.dll', 'rb').read()
e = struct.unpack_from('<I', data, 0x3C)[0]
nsec = struct.unpack_from('<H', data, e + 6)[0]
optsz = struct.unpack_from('<H', data, e + 20)[0]
secs = []
for i in range(nsec):
    b = e + 24 + optsz + i * 40
    secs.append(struct.unpack_from('<IIII', data, b + 12))
def va2off(va):
    rva = va - 0x180000000
    for sva, svz, sraw, spr in secs:
        if sva <= rva < sva + max(svz, spr):
            return sraw + (rva - sva)
TA = list(data[va2off(0x1801cca60):va2off(0x1801cca60) + 256])
TB = list(data[va2off(0x1801ccb60):va2off(0x1801ccb60) + 256])

def enc(w, L, zpos):
    buf = bytearray(w)
    for p in zpos:
        buf[p] = 0
    b4 = b6 = 0xFF
    for i in range(L):
        idx = b6 ^ buf[i]
        b6 = b4 ^ TA[idx]
        b4 = TB[idx]
    v = ~((b4 << 8) | b6) & 0xFFFF
    return (v >> 8, v & 0xFF)

xu = json.load(open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json'))
reqs, resps = [], []
for r in xu:
    if len(r['out']) == 120 and r['out'][:2] == 'aa':
        reqs.append(bytes.fromhex(r['out']))
    if len(r['in']) == 120 and r['in'][:2] == 'aa':
        resps.append(bytes.fromhex(r['in']))

for L in (60, 58, 52, 12, 10):
    for zpos in [(6, 7, 8), (6, 7), (6, 7, 8, 9)]:
        for tag, frames in (('REQ ', reqs), ('RESP', resps)):
            ok = sum(1 for b in frames if enc(b, L, zpos) == (b[6], b[7]))
            print(f'L={L:3} z={zpos} {tag} {ok}/{len(frames)}')
    print()
