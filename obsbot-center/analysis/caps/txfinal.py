#!/usr/bin/env python3
"""Corrected extraction: control records have URB at +16 {dir,ep_lo,ep_hi=08,dir}.
Setup = d[28:36]. Payload OUT = d[36:36+wL]. Pair submit->completion via next rec
(same IRP d[0:8]) whose payload IN = d[28:28+dlen].
Dump every vendor XU control transaction (bmt 0x21/0xa1/0x23/0xa3) as JSON."""
import struct, json
from collections import Counter
data = open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/cap1.pcap', 'rb').read()
pos = 24
recs = []
while pos + 16 <= len(data):
    t1, t2, cap, orig = struct.unpack_from('<IIII', data, pos)
    if cap > 0x10000:
        break
    recs.append((pos + 16, cap))
    pos += 16 + cap

def is_ctrl_out(d):
    return len(d) >= 44 and d[14] == 9 and d[16] == 1 and d[17] == 0 and d[18] == 0x08

def is_ctrl_in(d):
    return len(d) >= 40 and d[14] == 9 and d[16] == 0 and d[17] == 0 and d[18] == 0x08

txns = []
bmtc = Counter()
for i, (off, cap) in enumerate(recs):
    d = data[off:off + cap]
    if not is_ctrl_out(d):
        continue
    setup = d[28:36]
    bmt, breq = setup[0], setup[1]
    if bmt not in (0x21, 0xa1, 0x23, 0xa3, 0x41, 0xc1):
        bmtc[f'bmt{bmt:02x}'] += 1
        continue
    wV = struct.unpack_from('<H', setup, 2)[0]
    wI = struct.unpack_from('<H', setup, 4)[0]
    wL = struct.unpack_from('<H', setup, 6)[0]
    outpay = bytes(d[36:36 + wL]) if bmt in (0x21, 0x23, 0x41) else b''
    # completion(s): scan forward up to 8 recs for same IRP
    irp = d[0:8]
    inpay = b''
    for j in range(i + 1, min(i + 9, len(recs))):
        o2, c2 = recs[j]
        d2 = data[o2:o2 + c2]
        if len(d2) >= 8 and d2[0:8] == irp and d2[14] == 9 and is_ctrl_in(d2):
            dl = struct.unpack_from('<H', d2, 20)[0]
            inpay = bytes(d2[28:28 + dl])
            break
    bmtc[f'bmt{bmt:02x}_breq{breq:02x}_wV{wV:04x}'] += 1
    txns.append({'rec': i, 'bmt': bmt, 'breq': breq, 'wV': wV, 'wI': wI, 'wL': wL,
                 'out': outpay.hex(), 'in': inpay.hex()})
print('classes:', dict(bmtc))
print('vendor txns:', len(txns))
json.dump(txns, open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/txfinal.json', 'w'))
for t in txns[:25]:
    print(f"rec{t['rec']:5} {t['bmt']:02x} {t['breq']:02x} wV={t['wV']:04x} wI={t['wI']:04x} wL={t['wL']:3} "
          f"out={t['out'][:24]} in={t['in'][:40]}")
