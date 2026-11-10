#!/usr/bin/env python3
"""Pin the EXACT variant: for each of the 26 aa frames find every (zero-mask,
init, span, store-order) combo that reproduces bytes[6:8]. Report consistent
settings per byte1 class."""
import struct, json
from itertools import product
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

tx = json.load(open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json'))
frames = []
for t in tx:
    for key in ('out', 'in'):
        h = t.get(key) or ''
        if len(h) == 120 and h[:2] == 'aa':
            frames.append(bytes.fromhex(h))

variants = {}
for w in frames:
    got = (w[6], w[7])
    hits = []
    for z in ([6, 7], [6, 7, 8], [6, 7, 8, 9]):
        for init in ((0xFF, 0xFF), (0, 0), (0xFF, 0), (0, 0xFF)):
            for span in (12, 60):
                b = bytearray(w)
                for k in z:
                    b[k] = 0
                b4, b6 = init
                for i in range(span):
                    idx = b6 ^ b[i]
                    b6 = b4 ^ A[idx]
                    b4 = B[idx]
                v = ~((b4 << 8) | b6) & 0xFFFF
                if (v >> 8, v & 0xFF) == got:
                    hits.append((tuple(z), init, span, 'BE'))
                if (v & 0xFF, v >> 8) == got:
                    hits.append((tuple(z), init, span, 'LE'))
    key = w[1]
    variants.setdefault(key, []).append(hits)
for key, hs in sorted(variants.items()):
    from collections import Counter
    c = Counter()
    for hits in hs:
        for h in hits:
            c[h] += 1
    # combos that hit on EVERY frame of the class
    full = [h for h, n in c.items() if n == len(hs)]
    print(f'class 0x{key:02x} n={len(hs)}: full-hit variants =', full if full else 'NONE')
