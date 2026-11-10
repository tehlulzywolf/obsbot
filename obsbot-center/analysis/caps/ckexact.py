#!/usr/bin/env python3
"""EXACT replay of FUN_1800ffce0 checksum:
  u   = BE16(wire[1..2]);  span = u & 0xFFF
  buf = frame in 256B zero buffer; during compute buf[6]=buf[7]=buf[8]=0
  b4=b6=0xFF; for i in range(span): idx=b6^buf[i]; b6=b4^Ta[idx]; b4=Tb[idx]
  v = ~((b4<<8)|b6); wire[6]=v>>8; wire[7]=v&0xFF
Tables at VA 0x1801cca60 / 0x1801ccb60 via PE section map.
Variant flag: byte8 zero vs kept (device may keep it for resp)."""
import json, struct
data = open('/mnt/c/Program Files/OBSBOT Center/bin/libdev.dll', 'rb').read()
e = struct.unpack_from('<I', data, 0x3C)[0]
nsec = struct.unpack_from('<H', data, e + 6)[0]
optsz = struct.unpack_from('<H', data, e + 20)[0]
secs = []
for i in range(nsec):
    b = e + 24 + optsz + i * 40
    secs.append(struct.unpack_from('<IIII', data, b + 12))
def va2off(va):
    rva = va - 0x180000000
    for sva, svz, sraw, spr in secs:
        if sva <= rva < sva + max(svz, spr):
            return sraw + (rva - sva)
TA = list(data[va2off(0x1801cca60):va2off(0x1801cca60) + 256])
TB = list(data[va2off(0x1801ccb60):va2off(0x1801ccb60) + 256])
print('TA[:4]', [hex(x) for x in TA[:4]], 'TB[:4]', [hex(x) for x in TB[:4]])

def enc(w, keep8=False):
    u = (w[1] << 8) | w[2]
    span = u & 0xFFF
    buf = bytearray(256)
    buf[0:60] = w
    buf[6] = buf[7] = 0
    if not keep8:
        buf[8] = 0
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
print('spans req:', sorted({((b[1] << 8) | b[2]) & 0xFFF for b in reqs}),
      'resp:', sorted({((b[1] << 8) | b[2]) & 0xFFF for b in resps}))

for tag, frames, keep in (('REQ  keep0', reqs, False), ('REQ  keep8', reqs, True),
                          ('RESP keep0', resps, False), ('RESP keep8', resps, True)):
    ok = 0
    for b in frames:
        got = enc(b, keep)
        if got == (b[6], b[7]):
            ok += 1
    print(f'{tag}: {ok}/{len(frames)}')
