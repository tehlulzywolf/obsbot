#!/usr/bin/env python3
"""Raw DLT_USBPCAP parser for cap1.pcap — no tshark field dependency.
usbpcap_packet header: u32 length; u32 status; u64 irp_id; u8 function;
u8 config/endpoint; u8 interface; u8 reserve; u32 uralloc? ... Use 24-byte
header per Wireshark packet-usbpcap: fields:
  0-3   packet header (len<<0 | status<<16) — actually u32 len, u32 status
  0-3   u32 sequence? Let's parse per Wireshark docs:
  struct usbpcap_packet {
    uint32_t header_len_status; // len:16? no — full u32 length + u32 status
  }
We parse empirically: u32 packet_len, u32 status, u64 irp_id, u8 function,
u8 endpoint_address, u8 bus? ... then setup/data. Verify: function codes
URB_SUBMIT=0x01, URB_COMPLETE=0x02, URB_STATUS? Windows: 1=submit,2=complete.
"""
import struct, sys, json, re
from collections import defaultdict

PCAP = '/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/cap1.pcap'
out = []
with open(PCAP, 'rb') as f:
    g = f.read(24)
    magic = g[:4]
    if magic == b'\xd4\xc3\xb2\xa1': endian = '<'
    elif magic == b'\xa1\xb2\xc3\xd4': endian = '>'
    else: sys.exit('bad magic ' + magic.hex())
    vmaj, vmin, tz, sig, snaplen, network = struct.unpack(endian + 'HHiIII', g[4:24])
    print('linktype', network, 'endian', endian, file=sys.stderr)
    n = 0
    while True:
        ph = f.read(16)
        if len(ph) < 16: break
        ts_s, ts_u, caplen, origlen = struct.unpack(endian + 'IIII', ph)
        data = f.read(caplen)
        if len(data) < caplen: break
        n += 1
        if network != 249 or caplen < 24: continue
        # usbpcap header (24 bytes)
        pkt_len, status, irp_id = struct.unpack(endian + 'IIQ', data[0:16])
        (function, endpoint, transfer_flags, reserved,
         bus, device, setup0, setup1, setup2) = struct.unpack(endian + 'BBBBBBIQQ', data[16:44]) if caplen >= 44 else (None,)*9
        if caplen < 44:
            (function, endpoint, transfer_flags, reserved, bus, dev) = struct.unpack(endian + 'BBBBBB', data[16:22])
            setup0 = setup1 = setup2 = 0
        else:
            dev = device
        if dev != 9: continue
        out.append({'frame': n, 't': ts_s + ts_u/1e6, 'fn': function, 'ep': endpoint,
                    'xfer': transfer_flags, 'status': status, 'irp': irp_id,
                    'raw': data[24:].hex(), 'caplen': caplen})
print('dev9 records:', len(out), file=sys.stderr)
json.dump(out, open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/urb_raw9.json', 'w'))
# quick histogram of function codes and raw layout of first submit w/ setup
fc = defaultdict(int)
for r in out: fc[(r['fn'], r['ep'], r['xfer'])] += 1
for k in sorted(fc): print('fn=%d ep=%02x xfer=%d : %d' % (k[0], k[1], k[2], fc[k]), file=sys.stderr)
for r in out[:6]:
    print(r['frame'], hex(r['fn']), 'ep', hex(r['ep']), 'xfer', r['xfer'], 'cap', r['caplen'], r['raw'][:80], file=sys.stderr)
