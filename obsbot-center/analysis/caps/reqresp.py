#!/usr/bin/env python3
"""Pair req->resp (GET CUR immediately after SET CUR) and diff every offset.
Also map which req byte equals which resp byte at each offset -> reveals
swapped/echoed fields (addr, seq, flags)."""
import json
xu = json.load(open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json'))
# build chronological stream
seq_tx = sorted(xu, key=lambda r: r['fr'])
pairs = []
for i, r in enumerate(seq_tx):
    if r['bmt'] == 0x21 and len(r['out']) == 120 and r['out'][:2] == 'aa':
        # look ahead up to 4 tx for the GET CUR response
        for j in range(i + 1, min(i + 5, len(seq_tx))):
            s = seq_tx[j]
            if s['bmt'] == 0xa1 and len(s['in']) == 120 and s['in'][:2] == 'aa':
                pairs.append((r, s))
                break
            if s['bmt'] == 0xa1 and len(s['in']) == 120:
                pairs.append((r, s))
                break
print('pairs:', len(pairs))
for r, s in pairs[:6]:
    b, c = bytes.fromhex(r['out']), bytes.fromhex(s['in'])
    print(f"\nf{r['fr']} req : {' '.join(f'{x:02x}' for x in b[:20])}")
    print(f"f{s['fr']} resp: {' '.join(f'{x:02x}' for x in c[:20])}")
    diff = [(i, b[i], c[i]) for i in range(20) if b[i] != c[i]]
    print('  diff[0:20]:', [(i, f'{x:02x}', f'{y:02x}') for i, x, y in diff])
    # find swaps: c[i]==b[j]
    print('  resp[6:8] appears in req at:', [i for i in range(60) if b[i:i+2] == c[6:8]])
    print('  resp[8:10] appears in req at:', [i for i in range(60) if b[i:i+2] == c[8:10]])
    print('  req[6:8] appears in resp at:', [i for i in range(60) if c[i:i+2] == b[6:8]])
