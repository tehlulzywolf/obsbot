#!/usr/bin/env python3
"""Ground truth v2: find EVERY AA vendor control payload. The app sends:
  GET LEN (a1 85 wL=2) -> 3c00 ; SET CUR (21 01) 60B OUT ; GET CUR (a1 01 wL=60) -> IN.
OUT submits: setup at some offset then 60B payload (aa 01 ..).
Strategy: scan every record; for any 60-byte window with w[0]==0xaa and
w[1] in {0x01,0x29} and w[4]==0x0c and w[5]==0 and w[3]==0 and w[2]==seq-ish, dump
16B of context before/after. Then try checksums per FUN_1800ffce0 exact loop:
 span = BE16(w[1],w[2])&0xFFF; buf = wire w/ [6]=[7]=0; byte8 variants: keep/zero/
 chain-table (FUN_180163670: c=T[c^b] desc over buf[11..0], init 0xFF, T=DAT_1801cc960)
 tables TA=0x1801cca60 (lo update) TB=0x1801ccb60; store byte6=hi,byte7=lo of ~v."""
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

dll = open('/mnt/c/Program Files/OBSBOT Center/bin/libdev.dll', 'rb').read()
e = struct.unpack_from('<I', dll, 0x3C)[0]
nsec = struct.unpack_from('<H', dll, e + 6)[0]
optsz = struct.unpack_from('<H', dll, e + 20)[0]
secs = []
for i in range(nsec):
    b = e + 24 + optsz + i * 40
    secs.append(struct.unpack_from('<IIII', dll, b + 12))
def va2off(va):
    rva = va - 0x180000000
    for sva, svz, sraw, spr in secs:
        if sva <= rva < sva + max(svz, spr):
            return sraw + (rva - sva)
TA = list(dll[va2off(0x1801cca60):va2off(0x1801cca60) + 256])
TB = list(dll[va2off(0x1801ccb60):va2off(0x1801ccb60) + 256])
TC = list(dll[va2off(0x1801cc960):va2off(0x1801cc960) + 256])  # chain table

def chain(buf, init=0xFF):
    c = init
    for i in (11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0):
        c = TC[c ^ buf[i]]
    return c

def checksum(w, mode):
    buf = bytearray(w[0:60])
    buf[6] = buf[7] = 0
    if mode == 'keep8':
        pass
    elif mode == 'zero8':
        buf[8] = 0
    elif mode == 'chain8':
        buf[8] = chain(buf)
    span = ((w[1] << 8) | w[2]) & 0xFFF
    if span < 8 or span > 4096:
        span = 60
    buf += bytearray(4096)
    b4 = b6 = 0xFF
    for i in range(span):
        idx = b6 ^ buf[i]
        b6 = b4 ^ TA[idx]
        b4 = TB[idx]
    v = ~((b4 << 8) | b6) & 0xFFFF
    return (v >> 8, v & 0xFF)

# collect frames
frames = []  # (rec, off, kind, bytes60)
for ri, (off, cap) in enumerate(recs):
    d = data[off:off + cap]
    if len(d) < 46 or d[14] != 9:
        continue
    j = d.find(b'\xaa', 20)
    while j != -1 and j + 60 <= cap:
        w = d[j:j + 60]
        if w[1] in (0x01, 0x29) and w[2] < 60 and w[3] == 0 and w[4] == 0x0c and w[5] == 0x00:
            frames.append((ri, j, 'OUT' if w[1] == 0x01 else 'IN', w))
        j = d.find(b'\xaa', j + 1)
print('candidate vendor frames:', len(frames))
kinds = Counter(f[2] for f in frames)
print('by dir:', dict(kinds))
for (ri, j, kd, w) in frames[:6]:
    print(f'{kd} rec{ri}@{j}: {w[:16].hex(" ")}')

for mode in ('keep8', 'zero8', 'chain8'):
    ok = bad = 0
    firstbad = None
    for (ri, j, kd, w) in frames:
        got = checksum(w, mode)
        if got == (w[6], w[7]):
            ok += 1
        else:
            bad += 1
            if firstbad is None:
                firstbad = (kd, w, got)
    print(f'{mode}: OK={ok} BAD={bad}')
    if firstbad and ok == 0:
        kd, w, got = firstbad
        print('  first bad:', kd, w[:12].hex(' '), 'got', got, 'want', (w[6], w[7]))
