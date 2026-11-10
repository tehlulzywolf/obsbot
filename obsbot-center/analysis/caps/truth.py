#!/usr/bin/env python3
"""Ground truth layout pass. USBPcap DLT_USBPCAP: packet hdr 16B + URB hdr 24B.
URB layout (usbpcap_driver): ushub_port, bus, device, setup_packet[8], data_length,
staging/flags, ... Control submit carries SETUP + OUT data back-to-back at payload.
Enumerate for bus1/dev9 control records: print bmt,breq,wV,wI,wL, payload_len,
first 12 payload bytes hex, tagged OUT vs IN by wLength vs payload and direction."""
import struct
f = open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/cap1.pcap', 'rb')
magic = f.read(24)
print('pcap magic', magic[:4].hex())
fr = 0
ctrl = []
bulk = {}
while True:
    h = f.read(16)
    if len(h) < 16:
        break
    t1, t2, cap, orig = struct.unpack('<IIII', h)
    d = f.read(cap)
    if len(d) < cap:
        break
    fr += 1
    if cap < 40:
        continue
    if d[14] != 9:
        continue
    # try both 28- and 36-byte URB header guesses: setup at 28:36 worked before
    setup = d[28:36]
    bmt, breq = setup[0], setup[1]
    wV = setup[2] | (setup[3] << 8)
    wI = setup[4] | (setup[5] << 8)
    wL = setup[6] | (setup[7] << 8)
    kind = d[15]
    if bmt in (0x21, 0xa1, 0x23, 0xa3) and (breq & 0x0F) in (0x01, 0x03, 0x05):
        pl = d[36:]
        ctrl.append((fr, kind, bmt, breq, wV, wI, wL, bytes(pl)))
    elif d[14] == 9:
        bulk[kind] = bulk.get(kind, 0) + 1
print('ctrl records:', len(ctrl), 'bulk kinds:', bulk)
from collections import Counter
c = Counter((r[2], r[3], r[4], r[6]) for r in ctrl)
print('== distinct (bmt,breq,wV,wL) ==')
for k, v in c.most_common(20):
    print(f'  bmt={k[0]:02x} breq={k[1]:02x} wV={k[2]:04x} wL={k[3]} x{v}')
print('== first 26 control records (fr, kind, setup, payload head) ==')
for r in ctrl[:26]:
    fr, kind, bmt, breq, wV, wI, wL, p = r
    print(f'fr{fr} k{kind:02x} {bmt:02x} {breq:02x} wV{wV:04x} wI{wI:04x} wL{wL:3d} pay{len(p):3d} | {p[:14].hex(" ")}')
