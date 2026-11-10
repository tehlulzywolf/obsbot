#!/usr/bin/env python3
"""Locate pcap record containing abs offset 212904 (req0) and dump its raw bytes.
Then dump the whole record around both hits; show stride analysis of records near there."""
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
print('total recs walked:', len(recs), 'last end:', pos, 'file len:', len(data))

for target in (212904, 214892):
    # find record containing
    ri = None
    for i, (off, cap) in enumerate(recs):
        if off <= target < off + cap:
            ri = i
            break
    if ri is None:
        # binary check between records
        print(f'target {target}: NOT inside any walked record; nearby offsets:')
        import bisect
        offs = [o for (o, c) in recs]
        k = bisect.bisect_right(offs, target)
        for j in range(max(0, k - 3), min(len(recs), k + 2)):
            print('   rec', j, 'off', recs[j][0], 'cap', recs[j][1])
        continue
    off, cap = recs[ri]
    d = data[off:off + cap]
    print(f'target {target} in rec{ri} off{off} cap{cap} delta={target - off}')
    for r in range(max(0, target - off - 32), min(cap, target - off + 128), 16):
        print(f'  +{r:03x} ' + d[r:r + 16].hex(' '))
