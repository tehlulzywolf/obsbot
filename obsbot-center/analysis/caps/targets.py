#!/usr/bin/env python3
"""TX vendor frames: exact bytes around each of the 84 'aa 01 00' OUT hits
(pre 16B, frame 64B) + first 12 IN 'aa 29 00 00 0c 00' frames with surroundings.
These are the replay targets."""
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

print('=== OUT aa0100c00-ish exact contexts (first 14) ===')
n = 0
out_frames = []
for i, (off, cap) in enumerate(recs):
    d = data[off:off + cap]
    if len(d) < 46 or d[14] != 9:
        continue
    j = d.find(b'\xaa\x01\x00', 20)
    while j != -1 and n < 14:
        if d[j + 3] == 0x0c:  # req family
            pre = d[max(0, j - 24):j]
            post = d[j + 60:j + 84]
            print(f'rec{i} rec_cap{cap} pay@{j}')
            print('  pre :', pre.hex(' '))
            print('  f   :', d[j:j + 60].hex(' '))
            print('  post:', post.hex(' '))
            out_frames.append(d[j:j + 60])
            n += 1
        j = d.find(b'\xaa\x01\x00', j + 1)

print('=== IN aa2900c00-ish (first 12) ===')
n = 0
in_frames = []
for i, (off, cap) in enumerate(recs):
    d = data[off:off + cap]
    if len(d) < 46 or d[14] != 9:
        continue
    j = d.find(b'\xaa\x29\x00', 20)
    while j != -1 and n < 12:
        if d[j + 3] == 0x0c:
            pre = d[max(0, j - 24):j]
            print(f'rec{i} rec_cap{cap} pay@{j}')
            print('  pre :', pre.hex(' '))
            print('  f   :', d[j:j + 60].hex(' '))
            in_frames.append(d[j:j + 60])
            n += 1
        j = d.find(b'\xaa\x29\x00', j + 1)
