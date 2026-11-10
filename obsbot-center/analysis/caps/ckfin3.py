#!/usr/bin/env python3
"""Per-frame: which span L (buffer zero-padded 256, [6][7][8]=0) reproduces wire ck?"""
import json
data = open('/mnt/c/Program Files/OBSBOT Center/bin/libdev.dll', 'rb').read()
T2 = list(data[0x1cb060:0x1cb160])
T1 = list(data[0x1cb160:0x1cb260])

def ck_run(buf, L):
    b4 = b6 = 0xFF
    for i in range(L):
        idx = b6 ^ buf[i]
        b6 = b4 ^ T2[idx]
        b4 = T1[idx]
    v = ~((b4 << 8) | b6) & 0xFFFF
    return (v >> 8, v & 0xFF)

xu = json.load(open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json'))
reqs, resps = [], []
for r in xu:
    if len(r['out']) == 120 and r['out'][:2] == 'aa':
        reqs.append(bytes.fromhex(r['out']))
    if len(r['in']) == 120 and r['in'][:2] == 'aa':
        resps.append(bytes.fromhex(r['in']))

from collections import Counter
def survey(frames, tag, z8):
    cnt = Counter()
    for b in frames:
        buf = bytearray(256); buf[0:60] = b
        buf[6] = buf[7] = 0
        if z8: buf[8] = 0
        want = (b[6], b[7])
        hits = [L for L in range(2, 257) if ck_run(buf, L) == want]
        cnt[tuple(hits[:6])] += 1
        if not hits:
            print(tag, 'no L for', b[:12].hex(' '), 'want', '%02x%02x' % want)
    print(tag, dict(cnt))

survey(reqs, 'REQ z8=True ', True)
survey(reqs, 'REQ z8=False', False)
survey(resps, 'RESP z8=True ', True)
survey(resps, 'RESP z8=False', False)
