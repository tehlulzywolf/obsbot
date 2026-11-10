#!/usr/bin/env python3
"""Definitive capture: all aa01.. control OUT payload bytes verbatim, 12B and 60B."""
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

seen = []
for i, (off, cap) in enumerate(recs):
    d = data[off:off + cap]
    if len(d) < 46 or d[14] != 9:
        continue
    j = d.find(b'\xaa\x01\x00\x00\x0c\x00', 20)
    while j != -1:
        seen.append((i, bytes(d[j:j + 12])))
        j = d.find(b'\xaa\x01\x00\x00\x0c\x00', j + 1)
print('12B req total:', len(seen))
uniq = {}
for (_, p) in seen:
    uniq.setdefault(p.hex(' '), 0)
    uniq[p.hex(' ')] += 1
for h, n in list(uniq.items())[:30]:
    print(f'x{n} {h}')

print('=== 60B aa01 full dumps (first 4) ===')
n = 0
for i, (off, cap) in enumerate(recs):
    d = data[off:off + cap]
    if len(d) < 46 or d[14] != 9:
        continue
    j = d.find(b'\xaa\x01\x00\x00', 20)
    while j != -1:
        L = d[j + 4] | (d[j + 5] << 8)
        if L >= 30 and j + L <= cap:
            print(f'rec{i} L={L} pre4={d[max(0,j-4):j].hex(" ")}')
            print('  ', d[j:j + min(L, 64)].hex(' '))
            n += 1
        j = d.find(b'\xaa\x01\x00\x00', j + 1)
        if n >= 4:
            break
    if n >= 4:
        break

print('=== all distinct IN aa29 frames w/ length field (first 10) ===')
seen2 = []
for i, (off, cap) in enumerate(recs):
    d = data[off:off + cap]
    if len(d) < 46 or d[14] != 9:
        continue
    j = d.find(b'\xaa\x29', 20)
    while j != -1 and len(seen2) < 10:
        L = d[j + 4] | (d[j + 5] << 8)
        if 8 < L < 512 and j + L <= cap:
            seen2.append((i, L, bytes(d[j:j + L])))
        j = d.find(b'\xaa\x29', j + 1)
for (i, L, p) in seen2:
    print(f'rec{i} L={L}: {p[:32].hex(" ")}')
