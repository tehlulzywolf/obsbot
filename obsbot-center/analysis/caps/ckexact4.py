#!/usr/bin/env python3
"""FINAL exact replay, 4KB zero buffer (spans are 256..2317):
buf = 4096 zeros; buf[0:60]=wire; buf[6]=buf[7]=0 (byte8 KEPT);
u=BE16(wire[1..2]); span=u&0xFFF; loop; v=~((b4<<8)|b6); expect (v>>8,v&0xFF)==(w6,w7)."""
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

def enc(w, zero8):
    buf = bytearray(4096)
    buf[0:60] = w
    buf[6] = buf[7] = 0
    if zero8:
        buf[8] = 0
    span = (((w[1] << 8) | w[2]) & 0xFFF)
    b4 = b6 = 0xFF
    for i in range(span):
        idx = b6 ^ buf[i]
        b6 = b4 ^ TA[idx]
        b4 = TB[idx]
    v = ~((b4 << 8) | b6) & 0xFFFF
    return (v >> 8, v & 0xFF)

xu = json.load(open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json'))
reqs, resps = [], []
for r in xu:
    if len(r['out']) == 120 and r['out'][:2] == 'aa':
        reqs.append(bytes.fromhex(r['out']))
    if len(r['in']) == 120 and r['in'][:2] == 'aa':
        resps.append(bytes.fromhex(r['in']))

for tag, frames in (('REQ ', reqs), ('RESP', resps)):
    for zero8 in (True, False):
        hits = 0
        for w in frames:
            got = enc(w, zero8)
            if got == (w[6], w[7]):
                hits += 1
        print(f'{tag} zero8={zero8}: {hits}/{len(frames)}')
