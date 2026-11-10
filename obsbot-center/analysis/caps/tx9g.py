#!/usr/bin/env python3
"""cap1.pcap ground truth v8 — adjacency pairing (IRP ptrs recycled).
Verified byte patterns (findoff.py):
  control submit : d[14]==0x1b ; setup=[28:36] (bmt@28 in 21/a1, wIndex=[32:34])
  completion     : d[14]==0x08 ; IN data=[28:cap] (1495:3c00 / 1507:aa29..)
  other submit   : d[14]==0x09 ; EP=[28]
Completes immediately follow their submit frame (1494->95, 96->97, 1506->07)."""
import struct, json
from collections import Counter

PCAP = '/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/cap1.pcap'
frs = []
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
        if caplen >= 28:
            frs.append((n, round(ts_s + ts_u / 1e6, 4), bytes(d), caplen))
print('frames:', len(frs))

xu, other_ctrl = [], []
ep6_ev, ep6_sub, iso_n = [], 0, 0
i, N = 0, len(frs)
while i < N:
    fr, t, d, cap = frs[i]
    k = d[14]
    if k == 0x1b and cap >= 36:
        bmt, breq = d[28], d[29]
        if bmt in (0x21, 0xa1) and breq in (1, 0x81, 0x85):
            wV = d[30] | d[31] << 8; wI = d[32] | d[33] << 8; wL = d[34] | d[35] << 8
            indata = b''
            if i + 1 < N and frs[i + 1][2][14] == 0x08:
                d2, cap2 = frs[i + 1][2], frs[i + 1][3]
                if cap2 >= 28:
                    indata = d2[28:cap2]
                i += 1
            rec = {'fr': fr, 't': t, 'bmt': bmt, 'breq': breq, 'wV': wV, 'wI': wI,
                   'wL': wL, 'out': d[36:cap].hex(), 'in': indata.hex()}
            (xu if wI in (0x0200, 0x0600) or wV in (0x0200, 0x0600) else other_ctrl).append(rec)
    elif k == 0x09 and cap >= 29:
        ep = d[28]
        if ep == 0x86:
            ep6_sub += 1
        elif ep == 0x81:
            iso_n += 1
    elif k == 0x08 and cap > 28:
        ind = d[28:cap]
        if 1 <= len(ind) <= 24 and ind[:1] == b'\xaa':
            ep6_ev.append({'fr': fr, 't': t, 'data': ind.hex()})
    i += 1

json.dump(xu, open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json', 'w'))
json.dump(ep6_ev, open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/ep6_events.json', 'w'))
print('XU:', len(xu), 'replied:', sum(1 for r in xu if r['in']),
      '| other ctrl:', len(other_ctrl), '| EP6 subs:', ep6_sub,
      'EP6 ev frames:', len(ep6_ev), '| isoc subs:', iso_n)
c = Counter((hex(r['bmt']), hex(r['breq']), hex(r['wV']), hex(r['wI']), r['wL'],
             'R' if r['in'] else '-') for r in xu)
print('=== XU tuples (…R=reply) ===')
for k, v in c.most_common(12):
    print('  bmt=%s breq=%s wV=%s wI=%s wL=%s %s x%d' % (k[0], k[1], k[2], k[3], k[4], k[5], v))
print('=== deduped XU exchanges ===')
seen = set(); shown = 0
for r in xu:
    tag = {1: 'SET CUR', 0x81: 'GET CUR', 0x85: 'GET LEN'}.get(r['breq'], hex(r['breq']))
    pat = (tag, hex(r['wV']), r['in'][:6], r['out'][:6])
    if pat in seen: continue
    seen.add(pat)
    print(f"f{r['fr']} {tag} wV={r['wV']:#06x} wI={r['wI']:#06x} wL={r['wL']} in={len(r['in'])//2}B")
    if r['out']: print('   OUT:', r['out'][:136])
    if r['in']: print('   IN :', r['in'][:136])
    shown += 1
    if shown >= 14: break
print('=== EP6 event frames first 8 ===')
for e in ep6_ev[:8]:
    print(' f', e['fr'], 'len', len(e['data']) // 2, e['data'][:80])
print('=== frame-body stats (aa.. bodies in xu) ===')
f1 = Counter(); f45 = Counter(); b9 = Counter(); cmdw = Counter(); lens = Counter()
for r in xu + []:
    for hexs in (r['out'], r['in']):
        if len(hexs) >= 24 and hexs[:2] == 'aa':
            b = bytes.fromhex(hexs)
            f1[b[1]] += 1; f45[(b[4], b[5])] += 1; b9[b[9]] += 1
            cmdw[(b[10], b[11])] += 1; lens[len(b)] += 1
print(' len:', lens.most_common(6))
print(' byte1:', f1.most_common(10))
print(' byte4-5:', f45.most_common(10))
print(' byte9:', b9.most_common(10))
print(' byte10-11:', cmdw.most_common(12))
