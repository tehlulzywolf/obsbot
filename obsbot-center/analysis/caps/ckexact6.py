#!/usr/bin/env python3
"""EXACT replay of FUN_1800ffce0 on wire frames from tx9.json.
Function input = pre-swap struct; recover by un-swapping wire pairs (1,2),(4,5),(10,11).
Then: u=LE16(buf[1..2]); buf[8]=0; buf[6]=buf[7]=0;
 if (u&0xf000)==0x2000: buf[8]=chain(param_1) [skip; never fires when span=u&0xfff=type]
 b4=b6=0xFF; span=u&0xfff: idx=b6^buf[i]; b6=b4^A[idx]; b4=B[idx]
 v=~(b4<<8|b6); store variants. A=VA 0x1801cca60, B=VA 0x1801ccb60."""
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

def enc(w60):
    buf = bytearray(w60)          # already full 60-byte wire frame
    # un-swap to recover the pre-swap struct the function saw
    buf[1], buf[2] = buf[2], buf[1]
    buf[4], buf[5] = buf[5], buf[4]
    buf[10], buf[11] = buf[11], buf[10]
    u = buf[1] | (buf[2] << 8)
    if (u & 0xF000) != 0x2000:
        # wire u16@1 is BE type/seq, not LE struct field: use wire bytes directly,
        # span = type nibble (loop count), byte8 left as-is
        u = (w[1] << 8) | w[2]
        span = u & 0xFFF
        buf = bytearray(w)
        buf[6] = buf[7] = 0
        b4 = b6 = 0xFF
        for i in range(span):
            idx = b6 ^ buf[i]
            b6 = b4 ^ A[idx]
            b4 = B[idx]
        v = ~((b4 << 8) | b6) & 0xFFFF
        return u, (v >> 8, v & 0xFF), (v & 0xFF, v >> 8)
    buf[8] = 0
    buf[6] = buf[7] = 0
    span = u & 0xFFF
    b4 = b6 = 0xFF
    for i in range(span):
        idx = b6 ^ buf[i]
        b6 = b4 ^ A[idx]
        b4 = B[idx]
    v = ~((b4 << 8) | b6) & 0xFFFF
    return u, (v >> 8, v & 0xFF), (v & 0xFF, v >> 8)

tx = json.load(open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json'))
stats = {}
for t in tx:
    for key in ('out', 'in'):
        h = t.get(key) or ''
        if len(h) < 120:
            continue
        w = bytes.fromhex(h[:120])
        if w[0] != 0xAA:
            continue
        kind = f'{key}_{w[1]:02x}'
        u, hi_lo, lo_hi = enc(w)
        st = stats.setdefault(kind, [0, 0, 0])
        st[1] += 1
        if (w[6], w[7]) == hi_lo:
            st[0] += 1
        if (w[6], w[7]) == lo_hi:
            st[2] += 1
for k, v in sorted(stats.items()):
    print(k, f'hi/lo={v[0]} lo/hi={v[2]} n={v[1]}')
