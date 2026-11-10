#!/usr/bin/env python3
"""cap1.pcap dev9 ground truth v4 — offsets confirmed from hdrprobe dump:
  hdr_len at [0:2] (27 or 28); setup at hdr_len..+8; OUT data follows setup.
  Control submit: setup[2:4] little-endian == 0x0200 and setup[0] in {0x21,0xa1}.
  Control complete-IN: shares IRP id with its submit; IN data at [32:].
  Bulk EP6 submit: submit[30]==0x86; its complete (same IRP) data at [24:].
  Isoc: everything else.
"""
import struct, json

PCAP = '/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/cap1.pcap'
frames = []
with open(PCAP, 'rb') as f:
    n = 0
    while True:
        ph = f.read(16)
        if len(ph) < 16: break
        ts_s, ts_u, caplen, _ = struct.unpack('<IIII', ph)
        d = f.read(caplen)
        if len(d) < caplen: break
        n += 1
        if caplen < 28: continue
        if d[14] != 9: continue
        frames.append((n, round(ts_s + ts_u / 1e6, 6), d, caplen))
print('dev9 frames:', len(frames))

ctrl_sub = {}   # irp -> dict
bulk_sub = {}   # irp -> fr
tx = []
bulk_ev = []
unmatched_ctrl_compl = 0
for fr, t, d, caplen in frames:
    irp = struct.unpack('<Q', d[0:8])[0]
    hlen = struct.unpack('<H', d[0:2])[0]
    # NOTE hdr_len overlaps irp: field [0:2]=1b/1c then irp at [8:16]? no—
    # from dump: 1c 00 20 68 cd 60 81 e7 -> [0:8] is one 8-byte field: irp
    # with low 2 bytes = 001c/001b? Use full u64 as key, hlen = val & 0xFFFF.
    hlen = irp & 0xFFFF
    if hlen not in (0x1b, 0x1c):
        continue
    key = irp >> 16
    if caplen >= hlen + 8 and struct.unpack('<H', d[hlen + 2:hlen + 4])[0] == 0x0200 and d[hlen] in (0x21, 0xa1):
        setup = d[hlen:hlen + 8]
        out = d[hlen + 8:caplen]
        ctrl_sub[key] = {'fr': fr, 't': t, 'setup': setup, 'out': out}
        continue
    if caplen >= 32 and d[30] == 0x86:
        bulk_sub[key] = fr
        continue
for fr, t, d, caplen in frames:
    irp = struct.unpack('<Q', d[0:8])[0]
    key = irp >> 16
    if key in ctrl_sub and caplen >= 32 and (d[12:16] != b'\x00\x00\x00\x00' or fr != ctrl_sub[key]['fr']):
        s = ctrl_sub[key]
        if fr == s['fr']:
            continue
        indata = d[32:caplen]
        setup = s['setup']
        wV = setup[2] | setup[3] << 8; wI = setup[4] | setup[5] << 8; wL = setup[6] | setup[7] << 8
        tx.append({'fr': s['fr'], 't': s['t'], 'bmt': setup[0], 'breq': setup[1], 'wV': wV,
                   'wI': wI, 'wL': wL, 'out': s['out'].hex(), 'in': indata.hex()})
    elif key in bulk_sub and fr != bulk_sub[key] and caplen > 24:
        bulk_ev.append({'fr': fr, 't': t, 'data': d[24:caplen].hex()})

tx.sort(key=lambda r: r['fr'])
json.dump(tx, open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json', 'w'))
json.dump(bulk_ev, open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/ep6_events.json', 'w'))
print('ctrl tx:', len(tx), ' bulk IN payloads:', len(bulk_ev))

from collections import Counter
c = Counter((r['bmt'], r['breq'], r['wV'], r['wI'], r['wL']) for r in tx)
print('=== ctrl tuples ===')
for k, v in c.most_common(12):
    print(f"  bmt={k[0]:#04x} breq={k[1]:#04x} wV={k[2]:#06x} wI={k[3]:#06x} wL={k[4]} x{v}")
print('=== exchanges (dedup by pattern, first occurrences) ===')
seen = set(); shown = 0
for r in tx:
    tag = {1: 'SET CUR', 0x81: 'GET CUR', 0x85: 'GET LEN'}.get(r['breq'], hex(r['breq']))
    pat = (tag, r['wV'], r['wI'], len(r['in']), r['in'][:4])
    if pat in seen: continue
    seen.add(pat)
    print(f"f{r['fr']} {tag} wV={r['wV']:#06x} wI={r['wI']:#06x} wL={r['wL']} inlen={len(r['in'])//2}")
    if r['out']: print('   OUT:', r['out'][:130])
    if r['in']: print('   IN :', r['in'][:130])
    shown += 1
    if shown >= 14: break
print('=== EP6 event frames: first 6 ===')
for e in bulk_ev[:6]:
    print(' f', e['fr'], e['data'][:80])
