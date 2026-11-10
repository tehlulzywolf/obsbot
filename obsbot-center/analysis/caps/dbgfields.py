#!/usr/bin/env python3
import struct
PCAP = '/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/cap1.pcap'
want = {1490, 1494, 1495, 1496, 1497, 1504, 1505, 1506, 1507, 1510, 1511}
with open(PCAP, 'rb') as f:
    f.read(24)
    n = 0
    while True:
        ph = f.read(16)
        if len(ph) < 16: break
        ts_s, ts_u, caplen, _ = struct.unpack('<IIII', ph)
        d = f.read(caplen)
        if len(d) < caplen: break
        n += 1
        if n in want:
            irp = struct.unpack('<Q', d[0:8])[0]
            f12 = struct.unpack('<I', d[12:16])[0]
            f8 = struct.unpack('<I', d[8:12])[0]
            print(f"fr{n} cap{caplen} irp={irp:016x} b8-11={f8:08x} b12-15={f12:08x} "
                  f"d16={d[16]:02x} d17={d[17]:02x} d24-31={d[24:32].hex()} d32-43={d[32:44].hex()}")
        if n > 1511: break
