#!/usr/bin/env python3
"""Definitive codec confirmation over ALL aa frames in tx9.json (60B).
Hypothesis H1: b4=b6=0xFF; over full 60B with bytes[6],[7] zeroed (+[8] for 0x29/0x21/0x24 kinds):
   idx=b6^buf[i]; b6=b4^A[idx]; b4=B[idx]; ck=~(b4<<8|b6) hi at [6], lo at [7].
H2: zero bytes[6,7,8] for every kind. Report per byte1 value."""
import struct, json
dll = open('/mnt/c/Program Files/OBSBOT Center/bin/libdev.dll', 'rb').read()
e = struct.unpack_from('<I', dll, 0x3C)[0]
nsec = struct.unpack_from('<H', dll, e + 6)[0]
optsz = struct.unpack_from('<H', dll, e + 20)[0]
def va2off(va):
    rva = va - 0x180000000
    for i in range(nsec):
        b = e + 24 + optsz + i * 40
        sva, svz, sraw, spr = struct.unpack_from('<IIII', dll, b + 12)
        if sva <= rva < sva + max(svz, spr):
            return sraw + (rva - sva)
A = list(dll[va2off(0x1801cca60):va2off(0x1801cca60) + 256])
B = list(dll[va2off(0x1801ccb60):va2off(0x1801ccb60) + 256])

def ck(buf):
    b4 = b6 = 0xFF
    for i in range(60):
        idx = b6 ^ buf[i]
        b6 = b4 ^ A[idx]
        b4 = B[idx]
    v = ~((b4 << 8) | b6) & 0xFFFF
    return (v >> 8, v & 0xFF)

tx = json.load(open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json'))
frames = []
for t in tx:
    for key in ('out', 'in'):
        h = t.get(key) or ''
        if len(h) == 120 and h[:2] == 'aa':
            frames.append(bytes.fromhex(h))
print('aa frames:', len(frames))
from collections import Counter
stat = Counter()
for w in frames:
    b1 = w[1]
    got = (w[6], w[7])
    hit = None
    for name, z8 in (('z67', False), ('z678', True)):
        for span in (12, 60):
            b = bytearray(w)
            b[6] = 0
            b[7] = 0
            if z8:
                b[8] = 0
            b4 = b6 = 0xFF
            for i in range(span):
                idx = b6 ^ b[i]
                b6 = b4 ^ A[idx]
                b4 = B[idx]
            v = ~((b4 << 8) | b6) & 0xFFFF
            if ((v >> 8, v & 0xFF) == got) or ((v & 0xFF, v >> 8) == got):
                hit = f'{b1:02x} {name} span{span}'
    stat[hit or f'{b1:02x} MISS'] += 1
for k, v in sorted(stat.items()):
    print(k, v)
