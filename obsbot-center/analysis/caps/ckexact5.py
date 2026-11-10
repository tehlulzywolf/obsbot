#!/usr/bin/env python3
"""Replay FUN_1800ffce0 EXACTLY: param_1 = 4096B buffer, wire frame copied in,
bytes[6]=bytes[7]=0 (bytes[8] left), pre-swap interpretation per decompile:
  uVar3 = LE16(buf[1..2])
  swap: buf[1],buf[2] = hi,lo ; buf[4],buf[5] = LE16(buf[4..5]) hi,lo ; buf[10],[11] swap
  then if (uVar3 & 0xFFF) != 0 -> span = uVar3 & 0xFFF else span=0x100
  bVar4=0xFF; bVar6=0xFF (init)
  loop i<span: idx=bVar6^buf[i]; bVar6=bVar4^TA[idx]; bVar4=TB[idx]
  v = ~(b6|b4<<8); byte6=v>>8? store hi at [6] lo at [7]
TA = VA 0x1801cca60 (b6 table? order ambiguous), TB = 0x1801ccb60. Test both orders.
Frames from ground truth (req 12B + resp 12B prefixes, byte-padded to 60)."""
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

def enc(w60, TA, TB, zero8, spanmode):
    buf = bytearray(4096)
    buf[0:60] = w60
    buf[6] = buf[7] = 0
    if zero8:
        buf[8] = 0
    u = buf[1] | (buf[2] << 8)
    # swap per decompile
    buf[2] = u & 0xFF
    buf[1] = (u >> 8) & 0xFF
    u45 = buf[4] | (buf[5] << 8)
    buf[5] = u45 & 0xFF
    buf[4] = (u45 >> 8) & 0xFF
    buf[10], buf[11] = buf[11], buf[10]
    span = (u & 0xFFF) if spanmode == 'u' else 0x100
    if span == 0:
        span = 0x100
    b4 = b6 = 0xFF
    for i in range(span):
        idx = b6 ^ buf[i]
        b6 = b4 ^ TA[idx]
        b4 = TB[idx]
    v = ~(b6 | (b4 << 8)) & 0xFFFF
    return (v >> 8, v & 0xFF)

import sys
sys.path.insert(0, '/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps')
tx = json.load(open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json'))
reqs = []
resps = []
for t in tx:
    for key, dest in (('out', reqs), ('in', resps)):
        h = t.get(key) or ''
        if len(h) >= 120:
            w = bytes.fromhex(h[:120])
            if w[0] == 0xAA and w[4] == 0x0c and w[5] == 0x00:
                dest.append(w)
print('req', len(reqs), 'resp', len(resps))
for name, frames in (('REQ', reqs), ('RESP', resps)):
    stats = {}
    for ta, tb, tn in ((A, B, 'AB'), (B, A, 'BA')):
        for zero8 in (True, False):
            for sm in ('u', '0x100'):
                ok = sum(1 for w in frames if enc(w, ta, tb, zero8, sm) == (w[6], w[7]))
                stats[f'{tn} z8={int(zero8)} span={sm}'] = ok
    print(name, {k: f'{v}/{len(frames)}' for k, v in stats.items() if v})
