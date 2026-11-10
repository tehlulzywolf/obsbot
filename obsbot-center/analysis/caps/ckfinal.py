#!/usr/bin/env python3
"""FINAL checksum verify: tables T_lo@0x1cb060 T_hi@0x1cb160 (CRC-8/0x07 split),
loop idx=lo^b; lo=hi^T_lo[idx]; hi=T_hi[idx]; span=full buffer; store ~hi@6 ~lo@7.
Per variant: which zero-masking makes all frames pass?"""
import json
data = open('/mnt/c/Program Files/OBSBOT Center/bin/libdev.dll', 'rb').read()
TL = list(data[0x1cb060:0x1cb160])
TH = list(data[0x1cb160:0x1cb260])

def run(buf, L):
    lo = hi = 0xFF
    for i in range(L):
        idx = lo ^ buf[i]
        lo = hi ^ TL[idx]
        hi = TH[idx]
    return (~hi & 0xFF, ~lo & 0xFF)   # decompile: *pbVar2(+7)=~lo? b6 gets >>8

xu = json.load(open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json'))
reqs, resps = [], []
for r in xu:
    if len(r['out']) == 120 and r['out'][:2] == 'aa':
        reqs.append(bytes.fromhex(r['out']))
    if len(r['in']) == 120 and r['in'][:2] == 'aa':
        resps.append(bytes.fromhex(r['in']))

import itertools
VARIANTS = {
    'zero67':   lambda b: (b[6], b[7]),
}
def test(frames, tag):
    results = {}
    for name, zpos in [('none', ()), ('z67', (6, 7)), ('z678', (6, 7, 8)),
                       ('z6789', (6, 7, 8, 9)), ('z8', (8,)), ('z2_5', (2, 3, 4, 5))]:
        for pad, L in ((0, 60), (256, 256)):
            ok = 0
            for b in frames:
                buf = bytearray(pad)
                buf[0:60] = b
                for p in zpos:
                    buf[p] = 0
                want = (b[6], b[7])
                got = run(buf, L)
                if got == want or got[::-1] == want:
                    ok += 1
            results[name + (f'@{L}' if pad else '')] = ok
    print(tag, results, 'n =', len(frames))

test(reqs, 'REQ ')
test(resps, 'RESP')
