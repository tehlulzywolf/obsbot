#!/usr/bin/env python3
"""Pair control records by IRP (d[0:8]). Print kinds + raw bytes for 4 control pairs,
then find SET_CUR wValue 0x0200 setups (hex 21 01 00 02 00 00 3c 00)."""
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

# group by IRP
groups = {}
order = []
for i, (off, cap) in enumerate(recs):
    d = data[off:off + cap]
    if len(d) < 28 or d[14] != 9:
        continue
    irp = d[0:8]
    if irp not in groups:
        groups[irp] = []
        order.append(irp)
    groups[irp].append((i, bytes(d[:40]), cap))

kinds = Counter()
for irp in order:
    g = groups[irp]
    if len(g) != 2:
        continue
    (i1, h1, c1), (i2, h2, c2) = g
    k1 = struct.unpack_from('<H', h1, 16)[0]
    k2 = struct.unpack_from('<H', h2, 16)[0]
    kinds[(k1, k2)] += 1
print('IRP-pair endpoint-pair kinds:', dict(kinds))

# control pairs: ep 0x100 / 0 -> print raw
shown = 0
for irp in order:
    g = groups[irp]
    if len(g) != 2:
        continue
    (i1, h1, c1), (i2, h2, c2) = g
    k1 = struct.unpack_from('<H', h1, 16)[0]
    if k1 not in (0x100, 0, 0x200, 0x101):
        continue
    off1, cap1 = recs[i1]
    off2, cap2 = recs[i2]
    d1 = data[off1:off1 + cap1]
    d2 = data[off2:off2 + cap2]
    print(f'--- pair rec{i1}(cap{cap1}) / rec{i2}(cap{cap2}) ep={k1:#x}')
    print(' submit:', d1[:min(cap1, 60)].hex(' '))
    print(' compl :', d2[:min(cap2, 60)].hex(' '))
    shown += 1
    if shown >= 6:
        break

# brute search for 21 01 setup anywhere
needle = bytes.fromhex('210100020000')
hits = 0
p = 0
while True:
    p = data.find(needle, p)
    if p == -1:
        break
    print('needle at abs', p)
    hits += 1
    p += 1
print('needle hits:', hits)
