#!/usr/bin/env python3
"""Replay raw pcap: is 'aa 01 00 00 0c 00' frame actually 12B or 60B OUT?
Print wLength + actual OUT payload length for the first few SET CUR,
plus full 12B headers of every distinct aa-frame length seen."""
import struct
f = open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/cap1.pcap', 'rb')
f.read(24)
fr = 0
recs = []  # (fr, d[14], d[28:36] setup, caplen bytes from 28)
while True:
    h = f.read(16)
    if len(h) < 16:
        break
    t1, t2, cap, orig = struct.unpack('<IIII', h)
    d = f.read(cap)
    if len(d) < cap:
        break
    fr += 1
    if len(d) >= 36 and d[14] == 0x1b and d[28] in (0x21, 0xa1):
        setup = d[28:36]
        bmt, breq, wV, wI, wL = struct.unpack('<BBBBHHH', setup)
        payload = d[36:36 + min(wL, cap - 36)] if cap > 36 else b''
        recs.append((fr, bmt, breq, wV, wI, wL, payload))
print('ctrl recs:', len(recs))
for (fr, bmt, breq, wV, wI, wL, p) in recs:
    if bmt == 0x21 and p[:2] == b'\xaa\x01':
        print(f'fr{fr} SET wV={wV:04x} wI={wI:04x} wL={wL} paylen={len(p)}')
        print('   ', p[:16].hex(' '), '...' if len(p) > 16 else '')
