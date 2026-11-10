#!/usr/bin/env python3
"""Dump every control SETUP + first 60B payload (dev9 only), chronologically."""
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
print('records:', len(recs))
import binascii
pat = binascii.unhexlify('210100020000')  # SET_CUR wValue=0x0200
pat2 = binascii.unhexlify('a10100020000')  # GET_CUR
n = 0
for i, (off, cap) in enumerate(recs):
    d = data[off:off + cap]
    if len(d) < 46 or d[14] != 9:
        continue
    j = d.find(pat)
    if j == -1:
        j = d.find(pat2)
    if j != -1:
        setup = d[j:j + 8]
        pay = d[j + 8:j + 8 + 64]
        wL = setup[6] | (setup[7] << 8)
        print(f'rec{i} kind={d[15]:02x} bmt={setup[0]:02x} breq={setup[1]:02x} wV={setup[2:4].hex()} wI={setup[4:6].hex()} wL={wL}')
        print('   pay:', pay.hex(' '))
        n += 1
        if n >= 16:
            break
print('shown:', n)
