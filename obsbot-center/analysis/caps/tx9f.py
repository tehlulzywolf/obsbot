#!/usr/bin/env python3
"""cap1.pcap dev9 ground truth v7 — offsets VERIFIED by findoff.py:
  control submit : setup @28 (wIndex u16 @30-31 in {0x0200,0x0600});
                   OUT data @36.. (SET CUR wL=60)
  completion     : marker b[27]==0x03; IN data @32..
  (submit/completion pair on u64 @0:8)
"""
import struct, json
from collections import Counter

PCAP = '/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/cap1.pcap'
subs, comps = {}, {}
frames = 0
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
        if caplen < 28 or d[0:2] != b'\x1c\x00': continue
        irp = struct.unpack('<Q', d[8:16])[0]
        fn = d[16]
        if fn == 0 and caplen >= 36 and d[28] in (0x21, 0xa1) and d[29] in (1, 0x81, 0x85) \
           and d[32:34] in (b'\x00\x02', b'\x00\x06'):
            subs[irp] = {'fr': n, 't': round(ts_s + ts_u / 1e6, 6), 'setup': d[28:36],
                         'out': d[36:caplen]}
        elif fn == 8 and irp in subs and irp not in comps:
            comps[irp] = {'fr': n, 'in': d[28:caplen]}
frames = n

xu = []
for irp, s in subs.items():
    st = s['setup']
    if st[0] not in (0x21, 0xa1): continue
    wV = st[2] | st[3] << 8; wI = st[4] | st[5] << 8; wL = st[6] | st[7] << 8
    c = comps.get(irp)
    xu.append({'fr': s['fr'], 't': s['t'], 'bmt': st[0], 'breq': st[1], 'wV': wV,
               'wI': wI, 'wL': wL, 'out': s['out'].hex(),
               'in': c['in'].hex() if c else ''})
xu.sort(key=lambda r: r['fr'])
json.dump(xu, open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json', 'w'))
print('XU exchanges:', len(xu), 'with replies:', sum(1 for r in xu if r['in']))
c = Counter((hex(r['bmt']), hex(r['breq']), hex(r['wV']), hex(r['wI']), r['wL']) for r in xu)
print('=== tuples ===')
for k, v in c.most_common(12):
    print('  bmt=%s breq=%s wV=%s wI=%s wL=%s x%d' % (k[0], k[1], k[2], k[3], k[4], v))
print('=== deduped first occurrences ===')
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
# frame-body statistics across SET CUR + GET CUR replies
print('=== request bodies (aa..) field stats ===')
import collections
f1 = collections.Counter(); f45 = collections.Counter(); b9 = collections.Counter()
cmdw = collections.Counter(); lens = collections.Counter()
for r in xu:
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
