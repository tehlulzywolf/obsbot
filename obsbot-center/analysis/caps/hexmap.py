#!/usr/bin/env python3
"""Hex-dump 400 bytes of the first 6 dev9 frames at fixed offsets to map
URB header fields definitively."""
import struct
f = open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/cap1.pcap', 'rb')
f.read(24)
fr = 0
shown = 0
kinds = {}
while shown < 6:
    h = f.read(16)
    if len(h) < 16:
        break
    t1, t2, cap, orig = struct.unpack('<IIII', h)
    d = f.read(cap)
    if len(d) < cap:
        break
    fr += 1
    if cap < 30 or d[14] != 9:
        continue
    k = d[15]
    kinds[k] = kinds.get(k, 0) + 1
    if kinds[k] <= (3 if k in (0x1b, 0x08) else 2) and shown < 6:
        shown += 1
        print(f'--- fr{fr} cap{cap} byte14=9 byte15={k:02x}')
        for off in range(0, min(cap, 160), 16):
            row = d[off:off + 16]
            print(f'  {off:04x}: ' + row.hex(' '))
print('kind counts (early):', {hex(a): b for a, b in kinds.items()})
