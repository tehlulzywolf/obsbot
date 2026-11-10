#!/usr/bin/env python3
"""Full: load DLL, brute ALL 2^16 polys for canonical 256xu16 LE tables present
in the binary, then test each table against observed checksums at [6:8]."""
import struct, json, sys

DLL = '/mnt/c/Program Files/OBSBOT Center/bin/libdev.dll'
data = open(DLL, 'rb').read()

xu = json.load(open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json'))
FRAMES = []
for r in xu:
    for hexs in (r['out'], r['in']):
        if len(hexs) == 120 and hexs[:2] == 'aa':
            FRAMES.append(bytes.fromhex(hexs))
print('frames:', len(FRAMES), file=sys.stderr)

def r16(x):
    r = 0
    for i in range(16):
        if x >> i & 1: r |= 1 << (15 - i)
    return r

# Build reference signatures cheaply: t[0..3] for both modes, then bytes-search.
print('scanning polys...', file=sys.stderr)
cands = []  # (poly, mode, offsets[])
for poly in range(1, 0x10000):
    # normal sig
    t1 = []
    for i in (0, 1, 2, 3):
        c = i << 8
        for _ in range(8):
            c = ((c << 1) ^ poly) & 0xFFFF if c & 0x8000 else (c << 1) & 0xFFFF
        t1.append(c)
    sig_n = struct.pack('<4H', *t1)
    # reflected sig
    t2 = []
    for i in (0, 1, 2, 3):
        c = r16(i << 8)
        for _ in range(8):
            c = (c >> 1) ^ poly if c & 1 else c >> 1
        t2.append(c)
    sig_r = struct.pack('<4H', *t2)
    pos = data.find(sig_n)
    while pos >= 0:
        if data[pos + 510:pos + 512] != b'\x00\x00':
            # t[255] check later via full verify; still try
            cands.append((poly, 'normal', pos))
        else:
            cands.append((poly, 'normal', pos))
        pos = data.find(sig_n, pos + 2)
    pos = data.find(sig_r)
    while pos >= 0:
        cands.append((poly, 'refl', pos))
        pos = data.find(sig_r, pos + 2)
print('raw candidates:', len(cands), file=sys.stderr)

# verify full table consistency
tables = []
for poly, mode, pos in cands:
    if pos + 512 > len(data): continue
    t = list(struct.unpack_from('<256H', data, pos))
    ok = True
    if mode == 'normal':
        for i in (0x80, 0xFF):
            c = i << 8
            for _ in range(8):
                c = ((c << 1) ^ poly) & 0xFFFF if c & 0x8000 else (c << 1) & 0xFFFF
            if c != t[i]: ok = False; break
    else:
        for i in (0x80, 0xFF):
            c = r16(i << 8)
            for _ in range(8):
                c = (c >> 1) ^ poly if c & 1 else c >> 1
            if c != t[i]: ok = False; break
    if ok and (poly, mode, pos) not in tables:
        tables.append((poly, mode, pos))
print('verified tables:', [(hex(p), m, hex(o)) for p, m, o in tables], file=sys.stderr)

def crc_n(t, buf, init):
    c = init
    for by in buf:
        c = ((c << 8) & 0xFFFF) ^ t[((c >> 8) ^ by) & 0xFF]
    return c
def crc_r(t, buf, init):
    c = init
    for by in buf:
        c = (c >> 8) ^ t[(c ^ by) & 0xFF]
    return c

spans = {'0:6+8:60': lambda b: b[0:6] + b[8:60], '8:60': lambda b: b[8:60],
         '0:60': lambda b: b[0:60], '10:60': lambda b: b[10:60],
         '9:60': lambda b: b[9:60], '12:60': lambda b: b[12:60],
         '0:2+8:60': lambda b: b[0:2] + b[8:60], '2:6+8:60': lambda b: b[2:6] + b[8:60],
         '8:60+0:6': lambda b: b[8:60] + b[0:6], '6:8 skip; full w/ zero ck': lambda b: b[0:6] + b'\0\0' + b[8:60]}
inits = [0x0000, 0xFFFF, 0x1D0F, 0x020A, 0x0A0D, 0x0C00, 0xAA01, 0x0A0A, 0xD0A0]
hits = []
for poly, mode, pos in tables:
    t = list(struct.unpack_from('<256H', data, pos))
    fn = crc_r if mode == 'refl' else crc_n
    for sname, sf in spans.items():
        for init in inits:
            for xorout in (0, 0xFFFF):
                for order in ('be', 'le'):
                    if all((fn(t, sf(b), init) ^ xorout) == int.from_bytes(b[6:8], 'big' if order == 'be' else 'little') for b in FRAMES):
                        hits.append((hex(poly), mode, sname, hex(init), hex(xorout), order))
print('HITS:', hits if hits else 'NONE')
