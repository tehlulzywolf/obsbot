#!/usr/bin/env python3
"""USBPcap URB classification per Wireshark packet-usbpcap.c.
Common hdr (LE):
  u64 irp            +0
  u32 status_code    +8
  u8  setup_request  +12 (URB_FUNC bitfield)
  u8  bus_number     +13
  u16 device_address +14
  u16 endpoint_number+16 (bit8 = direction)
  u16 transfer_buffer_length +18
  u16 data_length    +20
  u16 frame_length   +22
  u8  transfer_flags +24 (bit0 ISO, bit1 ERRSTALL, bit2 ERRPIPE, bit3 DBGPID)
  u8  pad            +25
  u32 tsud_low       +26? ... setup[8] follows if setup_request bit set.
Print full field map for dev9 records, classify by (setup_request, direction, ep, kind)."""
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

cnt = Counter()
samples = {}
for i, (off, cap) in enumerate(recs):
    d = data[off:off + cap]
    if len(d) < 26 or d[14] != 9:
        continue
    setup_rq = d[12]
    bus = d[13]
    dev = d[14]
    ep = struct.unpack_from('<H', d, 16)[0]
    tblen = struct.unpack_from('<H', d, 18)[0]
    dlen = struct.unpack_from('<H', d, 20)[0]
    flags = d[24] if len(d) > 24 else 0
    epn, direc = ep & 0xFF, (ep >> 8) & 1
    key = (setup_rq, direc, epn, flags)
    cnt[key] += 1
    if key not in samples:
        samples[key] = (i, cap, bytes(d[:40]))
print(f'{"setup_rq":8} {"dir":3} {"ep":4} {"flags":5} count   sample(rec, caplen, hdr40)')
for key in sorted(cnt, key=lambda k: -cnt[k]):
    sr, dr, epn, fl = key
    (i, cap, h) = samples[key]
    print(f'{sr:#04x}    {dr}   {epn:#04x} {fl:#04x}  {cnt[key]:6}  rec{i} cap{cap} {h.hex(" ")}')
