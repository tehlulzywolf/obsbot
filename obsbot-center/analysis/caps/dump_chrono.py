#!/usr/bin/env python3
import json
recs = json.load(open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/ctrl9.json'))
print("total records:", len(recs))
from collections import Counter
print(Counter(r['bmt'] for r in recs))
print()
print("=== CHRONOLOGICAL XU traffic (bmt 0x21 SET_CUR out / 0xa1 GET* in-request / responses) ===")
i = 0
for r in recs:
    if r['bmt'] in (0x21, 0xa1, 0x80, 0xc0, 0x40, 0x00):
        tag = {0x21: 'SET_CUR>', 0xa1: 'GET___>', 0x80: '<RESP  '}.get(r['bmt'], f"bmt{r['bmt']:#04x}")
        p = r['payload']
        print(f"[{i:03d}] {tag} wV={r['wValue']:#06x} wI={r['wIndex']:#06x} wL={r['wLen']:3d} plen={len(p)//2:2d}  {p[:120]}")
        i += 1
