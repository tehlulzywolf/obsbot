#!/usr/bin/env python3
import json
xu = json.load(open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json'))
for r in xu:
    o = r.get('out', '')
    i = r.get('in', '')
    print(f"fr{r['fr']} bmt={r['bmt']:02x} breq={r['breq']:02x} wV={r.get('wV',0):04x} "
          f"wI={r.get('wI',0):04x} wL={r.get('wL',0)} out[0:8]={o[:16]} in[0:10]={i[:20]}")
