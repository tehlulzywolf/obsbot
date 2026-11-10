#!/usr/bin/env python3
import struct
from collections import Counter
PCAP = '/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/cap1.pcap'
c14 = Counter(); c15 = Counter()
with open(PCAP, 'rb') as f:
    f.read(24)
    n = 0
    bad = 0
    while True:
        ph = f.read(16)
        if len(ph) < 16: break
        ts_s, ts_u, caplen, _ = struct.unpack('<IIII', ph)
        d = f.read(caplen)
        if len(d) < caplen: break
        n += 1
        if caplen >= 16:
            c14[d[14]] += 1; c15[d[15]] += 1
        else:
            bad += 1
        if n >= 40000: break
print('frames scanned:', n, 'short:', bad)
print('byte14 top:', c14.most_common(6))
print('byte15 top:', c15.most_common(6))
