#!/usr/bin/env python3
import struct
PCAP = '/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/cap1.pcap'
with open(PCAP, 'rb') as f:
    f.read(24)
    n = 0
    seen = {}
    while True:
        ph = f.read(16)
        if len(ph) < 16: break
        ts_s, ts_u, caplen, _ = struct.unpack('<IIII', ph)
        d = f.read(caplen)
        if len(d) < caplen: break
        n += 1
        if n in (1490, 1494, 1496, 1507, 1500) or (caplen >= 16 and n < 2000 and d[14:16] in ((b'\x00\x09', b'\x01\x09', b'\x09\x00') )):
            seen[n] = d[:20].hex()
        if n > 2600: break
for k in sorted(seen): print(k, seen[k])
