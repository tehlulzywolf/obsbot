#!/usr/bin/env python3
"""Dump raw first 260 bytes of first 8 dev9 records to map USBPcap URB header."""
import struct
data = open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/cap1.pcap', 'rb').read()
pos = 24
recs = []
while pos + 16 <= len(data):
    t1, t2, cap, orig = struct.unpack_from('<IIII', data, pos)
    if cap > 0x10000:
        break
    recs.append((pos + 16, cap))
    pos += 16 + cap
shown = 0
for i, (off, cap) in enumerate(recs):
    d = data[off:off + cap]
    if len(d) < 20 or d[14] != 9:
        continue
    print(f'== rec{i} caplen={cap} ==')
    for r in range(0, min(cap, 96), 16):
        print(f'  +{r:03x} ' + d[r:r + 16].hex(' '))
    shown += 1
    if shown >= 8:
        break
