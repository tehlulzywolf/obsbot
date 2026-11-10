#!/usr/bin/env python3
"""Absolute byte search for the REAL captured frames:
  aa 01 00 00 0c 00 91 5c 0a 0d 08 18   (req0)
  aa 29 00 00 0c 00 a0 49               (resp prefix)
  aa 01 00 00                           (any req)
  aa 29 00 00                           (any resp)
  3c 00  (GET LEN response 2B: search '3c 00 00 00' near ctrl? skip)
Report all hit offsets."""
import binascii
data = open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/cap1.pcap', 'rb').read()
for name, hx in [
    ('req0 full12', 'aa0100000c00915c0a0d0818'),
    ('resp pre8  ', 'aa2900000c00a049'),
    ('aa010000   ', 'aa010000'),
    ('aa290000   ', 'aa290000'),
    ('aa2900     ', 'aa2900'),
    ('aa0100     ', 'aa0100'),
]:
    pat = binascii.unhexlify(hx)
    hits = []
    p = 0
    while len(hits) < 12:
        p = data.find(pat, p)
        if p == -1:
            break
        hits.append(p)
        p += 1
    print(f'{name} {hx}: {len(hits)} hits {hits[:8]}')
