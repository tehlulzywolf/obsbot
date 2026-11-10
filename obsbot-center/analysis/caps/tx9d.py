#!/usr/bin/env python3
"""cap1.pcap dev9 ground truth v5 — offsets CONFIRMED from busdev2 dump:
  [0:8]  irp (low16 = per-transfer seq, high = e781* pattern)
  [8:12] status ffffffff (submit)
  [12:16] function/flags (00000000 ctrl-submit, 00000009 bulk/isoc submit?,
          00000008 completion)
  [15]   device address (9)
  control submit: bytes24-27=00 01 00, setup=[28:36], OUT data=[36:cap]
  bulk/isoc submit: bytes24-26=00 01 00, byte27=endpoint(0x81/0x86)
  completion: [24:28] seq; control: setup-echo=[28:36], IN data=[36:cap]
              bulk/isoc: data=[24:cap]
Pair submit<->complete on irp u64."""
import struct, json

PCAP = '/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/cap1.pcap'
frames = []
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
        if caplen < 24 or d[14] != 9: continue
        frames.append((n, round(ts_s + ts_u / 1e6, 6), d, caplen))
print('dev9 frames:', len(frames))

ctrl_sub, bulk_sub, isoc_sub = {}, {}, {}
for fr, t, d, cap in frames:
    irp = struct.unpack('<Q', d[0:8])[0]
    fn = struct.unpack('<I', d[12:16])[0]
    if fn not in (0, 8, 9):
        continue
    if fn in (0,) and cap >= 36 and d[24] == 0 and d[25] == 1:
        setup = d[28:36]
        if setup[0] in (0x21, 0xa1, 0x80, 0x00, 0x20, 0x22, 0x23):
            ctrl_sub[irp] = {'fr': fr, 't': t, 'setup': setup, 'out': d[36:cap], 'seen_compl': False}
    elif fn == 9 and cap >= 28 and d[24] == 0 and d[25] == 1:
        ep = d[27]
        (bulk_sub if ep == 0x86 else isoc_sub)[irp] = fr

tx, ev6, iso = [], [], []
for fr, t, d, cap in frames:
    irp = struct.unpack('<Q', d[0:8])[0]
    fn = struct.unpack('<I', d[12:16])[0]
    if fn != 8: continue
    if irp in ctrl_sub and ctrl_sub[irp]['fr'] != fr:
        s = ctrl_sub[irp]
        setup = s['setup']
        wV = setup[2] | setup[3] << 8; wI = setup[4] | setup[5] << 8; wL = setup[6] | setup[7] << 8
        tx.append({'fr': s['fr'], 't': s['t'], 'bmt': setup[0], 'breq': setup[1], 'wV': wV,
                   'wI': wI, 'wL': wL, 'out': s['out'].hex(), 'in': d[36:cap].hex()})
    elif irp in bulk_sub and bulk_sub[irp] != fr:
        ev6.append({'fr': fr, 't': t, 'data': d[24:cap].hex()})
    elif irp in isoc_sub and isoc_sub[irp] != fr:
        iso.append(fr)

tx.sort(key=lambda r: r['fr'])
json.dump(tx, open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json', 'w'))
json.dump(ev6, open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/ep6_events.json', 'w'))
print('ctrl tx:', len(tx), ' EP6 IN events:', len(ev6), ' isoc compl:', len(iso))

from collections import Counter
c = Counter((r['bmt'], r['breq'], r['wV'], r['wI'], r['wL']) for r in tx)
print('=== ctrl tuples ===')
for k, v in c.most_common(12):
    print(f"  bmt={k[0]:#04x} breq={k[1]:#04x} wV={k[2]:#06x} wI={k[3]:#06x} wL={k[4]} x{v}")
print('=== deduped exchanges with payloads ===')
seen = set(); shown = 0
for r in tx:
    tag = {1: 'SET CUR', 0x81: 'GET CUR', 0x85: 'GET LEN'}.get(r['breq'], hex(r['breq']))
    pat = (tag, r['wV'], r['wI'], r['in'][:8], r['out'][:8])
    if pat in seen: continue
    seen.add(pat)
    print(f"f{r['fr']} {tag} wV={r['wV']:#06x} wI={r['wI']:#06x} wL={r['wL']} in={len(r['in'])//2}B")
    if r['out']: print('   OUT:', r['out'][:130])
    if r['in']: print('   IN :', r['in'][:130])
    shown += 1
    if shown >= 14: break
print('=== EP6 events first 6 ===')
for e in ev6[:6]:
    print(' f', e['fr'], 'len', len(e['data']) // 2, e['data'][:80])
