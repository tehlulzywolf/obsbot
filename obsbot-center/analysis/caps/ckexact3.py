#!/usr/bin/env python3
"""EXACT on 12-byte PRE-SWAP struct (decompile is authoritative):
struct[1..2] = LE 60; struct[4..5]=seq LE; struct[8]=0; struct[10..11]=cmd LE.
crc over struct[0..60) (args after 12, ck slot zeroed); store ~v at [6],[7]
=> wire bytes 6,7. Also try keep-byte8 + span variants."""
import json, struct as st
data = open('/mnt/c/Program Files/OBSBOT Center/bin/libdev.dll', 'rb').read()
e = st.unpack_from('<I', data, 0x3C)[0]
nsec = st.unpack_from('<H', data, e + 6)[0]
optsz = st.unpack_from('<H', data, e + 20)[0]
secs = []
for i in range(nsec):
    b = e + 24 + optsz + i * 40
    secs.append(st.unpack_from('<IIII', data, b + 12))
def va2off(va):
    rva = va - 0x180000000
    for sva, svz, sraw, spr in secs:
        if sva <= rva < sva + max(svz, spr):
            return sraw + (rva - sva)
TA = list(data[va2off(0x1801cca60):va2off(0x1801cca60) + 256])
TB = list(data[va2off(0x1801ccb60):va2off(0x1801ccb60) + 256])

def to_struct(w):
    s = bytearray(w[0:60])
    s[1], s[2] = w[2], w[1]      # LE length (assume true len 60 -> struct 3C 00)
    s[1], s[2] = 0x3C, 0x00
    s[4], s[5] = w[5], w[4]      # seq LE
    s[10], s[11] = w[11], w[10]  # cmd LE
    return s

def crc(buf, span, zpos):
    for p in zpos:
        buf[p] = 0
    b4 = b6 = 0xFF
    for i in range(span):
        idx = b6 ^ buf[i]
        b6 = b4 ^ TA[idx]
        b4 = TB[idx]
    return ~((b4 << 8) | b6) & 0xFFFF

xu = json.load(open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json'))
reqs, resps = [], []
for r in xu:
    if len(r['out']) == 120 and r['out'][:2] == 'aa':
        reqs.append(bytes.fromhex(r['out']))
    if len(r['in']) == 120 and r['in'][:2] == 'aa':
        resps.append(bytes.fromhex(r['in']))

for tag, frames in (('REQ ', reqs), ('RESP', resps)):
    for span in (60, 0x3C):
        pass
    res = {}
    for zpos in [(6, 7, 8), (6, 7)]:
        ok = 0
        for w in frames:
            s = to_struct(w)
            v = crc(s, 0x3C, zpos)
            if ((v >> 8) & 0xFF, v & 0xFF) == (w[6], w[7]):
                ok += 1
        res[str(zpos)] = ok
    print(tag, res, 'n=', len(frames))
