#!/usr/bin/env python3
"""cap1.pcap ground truth v6 — layout PROVEN by dbgfields.py dump:
  submit w/setup : d13=0x1b, setup=[28:36], OUT data [36:cap]
  submit no-data : d13=0x09, endpoint=[28] (0x81 isoc / 0x86 bulk-EP6)
  completion     : d13=0x08, IN data = [28:cap] (len cap-28)
Completes follow their submit as the next frame (verified 1494->95, 96->97,
1506->07). IRP ids are recycled kernel pointers; do not pair on them."""
import struct, json
from collections import Counter

PCAP = '/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/cap1.pcap'
fr_list = []
with open(PCAP, 'rb') as f:
    f.read(24)
    n = 0
    while True:
        ph = f.read(16)
        if len(ph) < 16: break
        ts_s, ts_u, caplen, _ = struct.unpack('<IIII', ph)
        d = f.read(caplen)
        if len(d) < caplen: break
        n += 1
        if caplen >= 29:
            fr_list.append((n, round(ts_s + ts_u / 1e6, 6), bytes(d), caplen))
print('frames:', len(fr_list))

tx = []          # control exchanges (submit + immediate completion IN)
ep6_ev = []      # EP6 completion payload frames
submits_ep6 = 0
i, N = 0, len(fr_list)
while i < N:
    fr, t, d, cap = fr_list[i]
    k = d[13]
    if k == 0x1b and cap >= 36:
        setup = d[28:36]
        bmt, breq = setup[0], setup[1]
        if bmt in (0x21, 0xa1, 0x80, 0x00, 0x20, 0x22, 0x23) and breq != 0:
            out = d[36:cap]
            indata = b''
            if i + 1 < N and fr_list[i + 1][2][13] == 0x08:
                d2, cap2 = fr_list[i + 1][2], fr_list[i + 1][3]
                if cap2 >= 28:
                    indata = d2[28:cap2]
                    i += 1
            wV = setup[2] | setup[3] << 8; wI = setup[4] | setup[5] << 8
            wL = setup[6] | setup[7] << 8
            tx.append({'fr': fr, 't': t, 'bmt': bmt, 'breq': breq, 'wV': wV,
                       'wI': wI, 'wL': wL, 'out': out.hex(), 'in': indata.hex()})
    elif k == 0x09 and cap >= 29 and d[28] == 0x86:
        submits_ep6 += 1
    elif k == 0x08 and cap > 28:
        indata = d[28:cap]
        if indata[:1] == b'\xaa' and len(indata) <= 20:
            ep6_ev.append({'fr': fr, 't': t, 'data': indata.hex()})
    i += 1

tx.sort(key=lambda r: r['fr'])
xu = [r for r in tx if r['wI'] in (0x0200, 0x0600) and r['wV'] in (0x0000, 0x0200, 0x0600, 0x0100)]
json.dump(tx, open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json', 'w'))
json.dump(ep6_ev, open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/ep6_events.json', 'w'))
print('ctrl exchanges:', len(tx), ' XU ones:', len(xu),
      ' EP6 submits:', submits_ep6, ' EP6 event frames:', len(ep6_ev))

c = Counter((hex(r['bmt']), hex(r['breq']), hex(r['wV']), hex(r['wI']), r['wL']) for r in tx)
print('=== ctrl tuples ===')
for k, v in c.most_common(12):
    print('  bmt=%s breq=%s wV=%s wI=%s wL=%s x%d' % (k[0], k[1], k[2], k[3], k[4], v))
print('=== XU exchanges, deduped ===')
seen = set(); shown = 0
for r in xu:
    tag = {1: 'SET CUR', 0x81: 'GET CUR', 0x85: 'GET LEN'}.get(r['breq'], hex(r['breq']))
    pat = (tag, r['wV'], r['in'][:6], r['out'][:6])
    if pat in seen: continue
    seen.add(pat)
    print(f"f{r['fr']} {tag} wV={r['wV']:#06x} wI={r['wI']:#06x} wL={r['wL']} in={len(r['in'])//2}B")
    if r['out']: print('   OUT:', r['out'][:136])
    if r['in']: print('   IN :', r['in'][:136])
    shown += 1
    if shown >= 14: break
print('=== EP6 events first 8 ===')
for e in ep6_ev[:8]:
    print(' f', e['fr'], 'len', len(e['data']) // 2, e['data'][:80])
