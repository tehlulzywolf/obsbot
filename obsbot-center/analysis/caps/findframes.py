#!/usr/bin/env python3
"""Find vendor frames with correct 6-byte header: aa dir seq 00 0c 00 (or len!=0c).
dir in 0x01..0x2f. Print rec, offset, full 60B context. Also count EP6 IN first bytes."""
import struct
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

hits = []
for i, (off, cap) in enumerate(recs):
    d = data[off:off + cap]
    if len(d) < 40 or d[14] != 9:
        continue
    j = d.find(b'\xaa', 20)
    while j != -1 and j + 12 <= cap:
        if d[j + 3] == 0 and d[j + 4] == 0x0c and d[j + 5] == 0x00 and d[j + 1] < 0x30:
            hits.append((i, j, bytes(d[j:j + 12])))
            j += 12
        else:
            j += 1
print('frame hits:', len(hits))
seqc = Counter(h[2][1] for h in hits)
print('dir/type byte distribution:', dict(seqc))
uniq = {}
for (i, j, p) in hits:
    uniq.setdefault(p.hex(' '), []).append((i, j))
print('unique frames:', len(uniq))
for h, locs in list(uniq.items())[:20]:
    print(f'x{len(locs)} rec{locs[0][0]}@{locs[0][1]}: {h}')
