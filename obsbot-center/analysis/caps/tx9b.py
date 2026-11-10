#!/usr/bin/env python3
"""cap1.pcap dev9 ground truth v3 (empirical offsets, two-pass IRP correlation).
Base header 24B. Submits (fn byte12 in {0,1}): control has setup at 28
(detected: bytes24-27 = 00 00 02 <len>, or setup bmRequestType known).
Completions (fn byte12 == 3): IN data starts 24 (or 32 for control, per
frame 1507: byte16=08 marker then data at 32)."""
import struct, json

PCAP = '/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/cap1.pcap'
submits = {}   # irp -> (fr, t, kind, header4, setuphex, outdata, xferlen)
frames = []    # (fr, t, irp, fn, d) dev9 all
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
        if caplen < 24: continue
        dev = d[23]
        if dev != 9: continue
        irp = struct.unpack('<Q', d[0:8])[0]
        fn = d[12]
        frames.append((n, round(ts_s + ts_u / 1e6, 6), irp, fn, d, caplen))

for fr, t, irp, fn, d, caplen in frames:
    if fn != 0 or caplen < 28: continue
    head4 = d[24:28]
    kind = None; setup = b''; out = b''
    if head4[0] == 0 and head4[1] == 0 and head4[2] == 2:
        kind = 'ctrl'; setup = d[28:36]; out = d[36:caplen]
    elif head4[:2] == b'\x00\x01' or head4[:2] == b'\x00\x00':
        # bulk (00 01) or isoc (00 00?) — endpoint in byte 16
        ep = d[16]
        kind = 'bulk' if ep in (0x06, 0x86, 0x02, 0x82, 0x04) else 'isoc'
        out = d[24:caplen]
    else:
        ep = d[16]
        kind = 'bulk' if ep in (0x06, 0x86) else 'isoc'
        out = d[24:caplen]
    submits.setdefault(irp, (fr, t, kind, head4, setup, out, caplen))

tx = []
ev = []
for fr, t, irp, fn, d, caplen in frames:
    if fn != 3: continue
    s = submits.get(irp)
    if s is None:
        continue
    sfr, st, kind, head4, setup, out, scap = s
    if kind == 'ctrl':
        indata = d[32:caplen] if caplen > 32 else b''
        wValue = setup[2] | setup[3] << 8; wIndex = setup[4] | setup[5] << 8; wLen = setup[6] | setup[7] << 8
        tx.append({'fr': sfr, 't': st, 'bmt': setup[0], 'breq': setup[1], 'wV': wValue,
                   'wI': wIndex, 'wL': wLen, 'out': out.hex(), 'in': indata.hex()})
    elif kind == 'bulk':
        indata = d[24:caplen] if caplen > 24 else b''
        if indata:
            ev.append({'fr': fr, 't': t, 'data': indata.hex()})

json.dump(tx, open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json', 'w'))
json.dump(ev, open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/ep6_events.json', 'w'))
print('ctrl tx:', len(tx), ' bulk IN payloads:', len(ev))
from collections import Counter
c = Counter((r['bmt'], r['breq'], r['wV'], r['wI'], r['wL'], bool(r['in'])) for r in tx)
print('=== ctrl tuples (bmt breq wV wI wL gotreply) ===')
for k, v in c.most_common(15):
    print(f"  bmt={k[0]:#04x} breq={k[1]:#04x} wV={k[2]:#06x} wI={k[3]:#06x} wL={k[4]:3d} reply={k[5]} x{v}")
print('=== control exchanges with replies ===')
shown = 0
for r in tx:
    if r['in']:
        tag = {1: 'SET', 0x81: 'GET', 0x85: 'LEN'}.get(r['breq'], hex(r['breq']))
        print(f"f{r['fr']} {tag} wV={r['wV']:#06x} wI={r['wI']:#06x} wL={r['wL']} in={len(r['in'])//2}B")
        if r['out']: print('   OUT:', r['out'][:120])
        print('   IN :', r['in'][:120])
        shown += 1
        if shown >= 16: break
