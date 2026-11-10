#!/usr/bin/env python3
"""TX-frame family study: find all 'aa <type> <seq>' frames in OUT data of dev9.
Print per-type: sample 3 frames full hex, count, and which byte positions change.
Determine true frame length by scanning forward for next aa-marker boundary."""
import struct, json
from collections import defaultdict
data = open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/cap1.pcap', 'rb').read()
pos = 24
recs = []
while pos + 16 <= len(data):
    t1, t2, cap, orig = struct.unpack_from('<IIII', data, pos)
    if cap > 0x10000:
        break
    recs.append((pos + 16, cap))
    pos += 16 + cap

fam = defaultdict(list)
for i, (off, cap) in enumerate(recs):
    d = data[off:off + cap]
    if len(d) < 46 or d[14] != 9:
        continue
    j = d.find(b'\xaa\x01\x00\x0c\x00', 20)   # req header signature
    while j != -1:
        fam['req'].append((i, j, bytes(d[j:j + 72])))
        j = d.find(b'\xaa\x01\x00\x0c\x00', j + 4)
    j = d.find(b'\xaa\x29\x00\x00\x0c\x00', 20)
    while j != -1:
        fam['resp'].append((i, j, bytes(d[j:j + 72])))
        j = d.find(b'\xaa\x29\x00\x00\x0c\x00', j + 4)
    # other types: aa XX where XX in 01..7f but NOT our two
    j = d.find(b'\xaa', 20)
    while j != -1 and j + 12 < cap:
        t = d[j + 1]
        if t not in (0x01, 0x29) and d[j:j + 4] != b'\xaa\x01\x00\x00':
            fam[f't{t:02x}'].append((i, j, bytes(d[j:j + 72])))
        j = d.find(b'\xaa', j + 1)

for key in sorted(fam, key=lambda k: -len(fam[k])):
    v = fam[key]
    if len(v) < 3:
        continue
    print(f'== family {key}: {len(v)} hits, first bytes 0..11 diversity:')
    sigs = defaultdict(int)
    for (_, _, p) in v[:200]:
        sigs[p[:6].hex()] += 1
    for s, n in list(sigs.items())[:5]:
        print('   ', s, 'x', n)
    for (i, j, p) in v[:2]:
        print(f'   rec{i}@{j}: {p[:32].hex(" ")}')
        print(f'           {p[32:72].hex(" ")}')
