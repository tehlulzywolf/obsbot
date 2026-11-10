#!/usr/bin/env python3
"""Empirically map DLT_USBPCAP header: scan offsets for values matching
bus=1 dev=9 known from tshark, on known dev9 control frames (1494-1507)."""
import struct

PCAP = '/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/cap1.pcap'
samples = {}
with open(PCAP, 'rb') as f:
    g = f.read(24)
    endian = '<' if g[:4] == b'\xd4\xc3\xb2\xa1' else '>'
    n = 0
    while True:
        ph = f.read(16)
        if len(ph) < 16: break
        ts_s, ts_u, caplen, origlen = struct.unpack(endian + 'IIII', ph)
        data = f.read(caplen)
        if len(data) < caplen: break
        n += 1
        if 1490 <= n <= 1512:
            samples[n] = data
        if n > 1512: break
for fn_, d in samples.items():
    print(fn_, 'cap', len(d), d[:40].hex())
