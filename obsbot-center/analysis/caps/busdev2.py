#!/usr/bin/env python3
import struct
PCAP = '/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/cap1.pcap'
want = {55, 63, 1490, 1492, 1494, 1495, 1496, 1497, 1500, 1506, 1507, 2000}
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
        if n in want:
            print(n, 'cap', caplen, d[:24].hex(), '|', d[24:40].hex() if caplen >= 40 else '')
        if n > 2000: break
