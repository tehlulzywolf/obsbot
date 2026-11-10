#!/usr/bin/env python3
import json
xu = json.load(open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json'))
seen = []
for r in xu:
    o, i = r['out'], r['in']
    if r['wV'] in (0x0400, 0x0600) or r.get('wI') in (0x0001, 0x0100, 0x0300) or len(o) not in (0, 120) or (len(i) not in (0, 2, 4, 120) and i):
        seen.append((r['fr'], f"{r['bmt']:02x}{r['breq']:02x}", f"wV={r.get('wV',0):04x} wI={r.get('wI',0):04x} wL={r.get('wL',0)}",
                     o[:120], i[:120]))
for s in seen:
    print(s[0], s[1], s[2])
    if s[3]: print('   OUT', ' '.join(s[3][k:k+2] for k in range(0, len(s[3]), 2)))
    if s[4]: print('   IN ', ' '.join(s[4][k:k+2] for k in range(0, len(s[4]), 2)))
