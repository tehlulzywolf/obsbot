#!/usr/bin/env python3
"""Readable digest of tx9.json + brute bitwise CRC-16 polys (no table) on 4 frames."""
import json, sys, struct

xu = json.load(open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json'))
sel2 = [r for r in xu if r['wI'] == 0x0200 or r['wI'] == 0x0600]

print('=== XU sel2/6 exchanges (fr kind wV wL -> reply) ===')
for r in sel2:
    kind = 'GETLEN' if (r['breq'] == 0x85 and r['wL'] == 2) else ('SET' if r['bmt'] == 0x21 else 'GET')
    inn = r['in'] or '-'
    print(f"f{r['fr']:6d} {kind:6s} sel={r['wI']>>8} wL={r['wL']:3d} out={r['out'][:24]:24s} in={inn[:24]}")

# frames for CRC brute: requests (60B, ck@6)
reqs, resps = [], []
for r in sel2:
    if r['bmt'] == 0x21 and len(r['out']) == 120:
        reqs.append(bytes.fromhex(r['out']))
    if len(r['in']) == 120 and r['in'][:2] == 'aa':
        resps.append(bytes.fromhex(r['in']))
allf = (reqs + resps)[:10]
print(f"\nreq60={len(reqs)} resp60={len(resps)}", file=sys.stderr)

def crc_bitwise(data, poly, init, refin, refout, xorout):
    def rb(x):
        r = 0
        for i in range(8):
            if x >> i & 1: r |= 1 << (7 - i)
        return r
    crc = init
    for by in data:
        if refin: by = rb(by)
        crc ^= by << 8
        for _ in range(8):
            crc = ((crc << 1) ^ poly) & 0xFFFF if crc & 0x8000 else (crc << 1) & 0xFFFF
    if refout:
        r = 0
        for i in range(16):
            if crc >> i & 1: r |= 1 << (15 - i)
        crc = r
    return crc ^ xorout

spans = {
    'full60': lambda b: b[0:60],
    'zeroCk': lambda b: b[0:6] + b'\0\0' + b[8:60],
    'noCk': lambda b: b[0:6] + b[8:60],
    'argsOnly': lambda b: b[8:60],
}
def has_magic(b): return b[0:2] == b'\xaa\x01' or b[0:2] in (b'\xaa\x29', b'\xaa\x89', b'\xaa\x09', b'\xaa\x81')

test = allf[:5]
def check(poly, refin, init, xorout, sf):
    for b in test:
        want_be = int.from_bytes(b[6:8], 'big')
        want_le = int.from_bytes(b[6:8], 'little')
        got = crc_bitwise(sf(b), poly, init, refin, refin, xorout)
        if got != want_be and got != want_le:
            return False
    return True

print('brute bitwise...', file=sys.stderr)
found = []
for poly in range(1, 0x10000):
    if poly == 1: continue
    for refin in (True, False):
        for init, xorout in ((0xFFFF, 0xFFFF), (0xFFFF, 0), (0, 0), (0, 0xFFFF)):
            for sname, sf in spans.items():
                if check(poly, refin, init, xorout, sf):
                    found.append((hex(poly), refin, hex(init), hex(xorout), sname))
                    print('MATCH:', found[-1])
print('bitwise results:', found if found else 'NONE')
print('sample reqs:')
for b in test[:4]:
    print(' ', b.hex())
