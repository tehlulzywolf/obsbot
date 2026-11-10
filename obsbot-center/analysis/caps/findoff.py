#!/usr/bin/env python3
"""Locate known byte patterns' offsets empirically, then parse generically."""
import struct
PCAP = '/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/cap1.pcap'
want = {1494: 'a1850002', 1495: '3c00', 1496: '21010002', 1497: '', 1506: 'a1810002', 1507: 'aa29', 1490: ''}
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
            hx = d.hex()
            pat = want[n]
            off = hx.find(pat) if pat else None
            print(f"fr{n} cap{caplen} pat={pat!r} at byte={off if off is None else off//2}")
            print('   full:', hx)
        if n > 1507: break
