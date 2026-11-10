#!/usr/bin/env python3
"""FUN_1800ffce0 TX checksum: buffer[256], frame bytes at [0:60],
b6=b7=b8=0; L=0x100 for wire 'aa 01 00'; CRC-16 via split tables; store ~BE."""
import json

data = open('/mnt/c/Program Files/OBSBOT Center/bin/libdev.dll', 'rb').read()
T2 = list(data[0x1cb060:0x1cb060 + 256])   # DAT_1801cca60
T1 = list(data[0x1cb160:0x1cb160 + 256])   # DAT_1801ccb60

def ck_calc(buf256, L):
    b4 = b6 = 0xFF
    for i in range(L):
        idx = b6 ^ buf256[i]
        b6 = b4 ^ T2[idx]
        b4 = T1[idx]
    v = ~((b4 << 8) | b6) & 0xFFFF
    return ((v >> 8) & 0xFF, v & 0xFF)

xu = json.load(open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json'))
reqs, resps = [], []
for r in xu:
    if len(r['out']) == 120 and r['out'][:2] == 'aa':
        reqs.append(bytes.fromhex(r['out']))
    if len(r['in']) == 120 and r['in'][:2] == 'aa':
        resps.append(bytes.fromhex(r['in']))

ok = bad = 0
for b in reqs:
    L = ((b[2] << 8) | b[1]) & 0xFFF          # struct LE: swapped wire b1,b2
    buf = bytearray(256)
    buf[0:60] = b
    buf[6] = buf[7] = buf[8] = 0
    got = ck_calc(buf, L)
    want = (b[6], b[7])
    if got == want:
        ok += 1
    else:
        bad += 1
        print('BAD', b[:12].hex(' '), f'L={L} want={want[0]:02x}{want[1]:02x} got={got[0]:02x}{got[1]:02x}')
print(f'TX checksum verified {ok}/{ok+bad}')

# derive the polynomial identity of the byte tables
def refl(v, n=16): return int(format(v, '016b')[::-1], 2)

for poly in range(0x10000):
    c = 0
    t1p = []
    for i in range(256):
        c = i
        for _ in range(8):
            c = (c >> 1) ^ poly if c & 1 else c >> 1
        t1p.append(c)
    if t1p[:8] == T1[:8]:
        print('T1/T2 = reflected CRC-8 poly', hex(poly))
        break
