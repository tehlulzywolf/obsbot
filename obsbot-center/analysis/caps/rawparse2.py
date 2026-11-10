#!/usr/bin/env python3
"""Raw DLT_USBPCAP parser v2 (fresh file).
Header: u32 packet_len, u32 status, u64 irp_id, u8 fn, u8 ep, u8 flags,
u8 reserved, u32 bus|dev<<16 (=24B); control submits append 8B setup then data."""
import struct, sys, json

PCAP = '/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/cap1.pcap'
recs = []
with open(PCAP, 'rb') as f:
    g = f.read(24)
    endian = '<' if g[:4] == b'\xd4\xc3\xb2\xa1' else '>'
    vmaj, vmin, tz, sig, snaplen, network = struct.unpack(endian + 'HHiIII', g[4:24])
    n = 0
    while True:
        ph = f.read(16)
        if len(ph) < 16: break
        ts_s, ts_u, caplen, origlen = struct.unpack(endian + 'IIII', ph)
        data = f.read(caplen)
        if len(data) < caplen: break
        n += 1
        if network != 249 or caplen < 24: continue
        pkt_len, status, irp = struct.unpack(endian + 'IIQ', data[0:16])
        fn, ep, flags, res = struct.unpack(endian + 'BBBB', data[16:20])
        busdev = struct.unpack(endian + 'I', data[20:24])[0]
        bus, dev = busdev & 0xFFFF, busdev >> 16
        if dev != 9: continue
        rec = {'fr': n, 't': round(ts_s + ts_u / 1e6, 6), 'fn': fn, 'ep': ep,
               'fl': flags, 'st': status, 'irp': irp, 'cap': caplen}
        xfer_type = flags & 0x07 if fn == 1 else None
        if fn == 1 and xfer_type == 2 and caplen >= 32:  # control submit
            rec['setup'] = data[24:32].hex()
            rec['data'] = data[32:caplen].hex()
            rec['plen'] = caplen - 32
        elif fn == 2 and caplen > 24:  # complete carries data
            rec['data'] = data[24:caplen].hex()
            rec['plen'] = caplen - 24
        recs.append(rec)

print('dev9 records:', len(recs))
from collections import Counter
c = Counter((r['fn'], (r['fl'] & 7) if r['fn'] == 1 else -1, r['ep']) for r in recs)
for k, v in sorted(c.items()):
    print(f"fn={k[0]} xfer={k[1]} ep={k[2]:#04x} : {v}")
json.dump(recs, open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/urb_raw9.json', 'w'))
withdata = [r for r in recs if r.get('plen', 0) > 0]
print('records with payload:', len(withdata))
for r in withdata[:6]:
    print(' fn', r['fn'], 'ep', hex(r['ep']), 'plen', r['plen'], 'setup', r.get('setup', '-'), 'data', r['data'][:70])
