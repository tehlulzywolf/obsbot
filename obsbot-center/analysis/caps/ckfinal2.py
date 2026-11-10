
import struct, json
dll = open('/mnt/c/Program Files/OBSBOT Center/bin/libdev.dll','rb').read()
e = struct.unpack_from('<I', dll, 0x3C)[0]
nsec = struct.unpack_from('<H', dll, e+6)[0]
optsz = struct.unpack_from('<H', dll, e+20)[0]
def va2off(va):
    rva = va - 0x180000000
    for i in range(nsec):
        b = e + 24 + optsz + i*40
        sva, svz, sraw, spr = struct.unpack_from('<IIII', dll, b+12)
        if sva <= rva < sva + max(svz, spr):
            return sraw + (rva - sva)
A = list(dll[va2off(0x1801cca60):va2off(0x1801cca60)+256])
B = list(dll[va2off(0x1801ccb60):va2off(0x1801ccb60)+256])
def ck(buf, span):
    lo = hi = 0xFF
    for i in range(span):
        idx = lo ^ buf[i]
        lo = hi ^ A[idx]
        hi = B[idx]
    return ~((hi<<8)|lo) & 0xFFFF
tx = json.load(open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json'))
rows = []
for t in tx:
    for key in ('out','in'):
        h = t.get(key) or ''
        if len(h) == 120 and h[:2] == 'aa':
            rows.append(bytes.fromhex(h))
from collections import Counter
res = Counter()
for w in rows:
    got = (w[6], w[7])
    for span in (12, 60):
        b = bytearray(w[:span]); b[6]=0; b[7]=0
        v = ck(b, span)
        tag = []
        if (v>>8, v&0xFF) == got: tag.append(f'span{span} BE')
        if (v&0xFF, v>>8) == got: tag.append(f'span{span} LE')
        if tag: res[(w[1], tuple(tag))] += 1
        else: res[(w[1], 'MISS')] += 1
for k, v in sorted(res.items()):
    print(hex(k[0]), k[1], v)
print()
print('the 89 frame:')
for w in rows:
    if w[1] == 0x89:
        print(w[:24].hex(' '), 'got', w[6:8].hex())
        b = bytearray(w[:12]); b[6]=0; b[7]=0
        print(' span12 BE', hex(ck(b,12)), 'LE', hex((ck(b,12)&0xFF)<<8 | ck(b,12)>>8))
