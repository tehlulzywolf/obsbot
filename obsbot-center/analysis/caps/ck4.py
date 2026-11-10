#!/usr/bin/env python3
"""Scan libdev.dll for canonical 256-entry u16 CRC tables; then brute the
checksum over ground-truth frames with every (table-pair, init, xorout, byte order)."""
import json, struct

DLL = '/mnt/c/Program Files/OBSBOT Center/bin/libdev.dll'
data = open(DLL, 'rb').read()
base = 0x180000000  # image base (headers); rdata vaddr 0x1801c1000 per earlier dump

# canonical check: table[0]==0 and table[1]==poly for normal; reflected t[1]==rev16(poly)
def rev16(x):
    r = 0
    for i in range(16):
        if x >> i & 1: r |= 1 << (15 - i)
    return r

def gen_table(poly, refl):
    def rf(x):
        r = 0
        for i in range(16):
            if x >> i & 1: r |= 1 << (15 - i)
        return r
    t = []
    for i in range(256):
        if refl: c = rev16(i << 8)
        else: c = i << 8
        for _ in range(8):
            c = ((c >> 1) ^ poly) if (c & 1) else (c >> 1) if refl else (((c << 1) ^ poly) & 0xFFFF if (c & 0x8000) else (c << 1))
        t.append(c)
    return t

found = {}
for off in range(0, len(data) - 516, 4):
    t = [struct.unpack_from('<H', data, off + 2 * i)[0] for i in range(256)]
    if t[0] != 0: continue
    if t[1] == 0: continue
    # verify table self-consistency for candidate poly
    p = t[1]
    # normal table: crc step (c<<1)^p
    ok = True
    for i in (0x80, 0xFF, 0x40):
        c = i << 8
        for _ in range(8):
            c = ((c << 1) ^ p) & 0xFFFF if c & 0x8000 else (c << 1) & 0xFFFF
        if c != t[i]: ok = False; break
    if ok:
        found[('normal', p)] = off; continue
    p2 = rev16(t[1])
    ok = True
    for i in (0x80, 0xFF, 0x40):
        c = rev16(i << 8)
        for _ in range(8):
            c = (c >> 1) ^ p2 if c & 1 else c >> 1
        if c != t[i]: ok = False; break
    if ok:
        found[('reflected', p2)] = off
for k, v in found.items():
    print('table:', k, 'off', hex(v), 'vaddr', hex(base + v))

# load the tables and brute
frames = []
for h in ['aa0100000c00915c0a0d081800000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000',
          'aa0101000c00c1500a0d080400000000000037be8bbcbd72000000000000000000000000000000000000000000000000000000000000000000000000',
          'aa0102000c00c0960a0d4819000000000000000000000000000037be8bbcbd7201000000000000000000000000000000000000000000000000000000',
          'aa0104000c00d6730a0242c4000000007000000000000000000027cb5c02000007000000000000000000000019000200600000000000000044210dc0',
          'aa2900000c00a0490d0a0818180059fc000000000000000000000000000000000000000000000000000037be8bbcbd72000000000000000000000000',
          'aa2902000c00f1830d0a48192200987f080008020106020600000000000000000000000000000000000037be8bbcbd72010000000000000000000000']:
    frames.append(bytes.fromhex(h))

def crc_normal(t, buf, init):
    c = init
    for by in buf:
        c = ((c << 8) & 0xFFFF) ^ t[((c >> 8) ^ by) & 0xFF]
    return c
def crc_refl(t, buf, init):
    c = init
    for by in buf:
        c = (c >> 8) ^ t[(c ^ by) & 0xFF]
    return c

for (mode, poly), off in found.items():
    t = [struct.unpack_from('<H', data, off + 2 * i)[0] for i in range(256)]
    fn = crc_refl if mode == 'reflected' else crc_normal
    spanfns = {'0:6+8:60': lambda b: b[0:6] + b[8:60], '8:60': lambda b: b[8:60],
               '0:60': lambda b: b[0:60], '10:60': lambda b: b[10:60],
               '9:60': lambda b: b[9:60], '12:60': lambda b: b[12:60],
               '0:2+8:60': lambda b: b[0:2] + b[8:60]}
    for sname, sf in spanfns.items():
        for init in (0x0000, 0xFFFF, 0x1D0F, 0x020A, 0x0A0D):
            for xorout in (0, 0xFFFF):
                for order in ('be', 'le'):
                    ok = True
                    for b in frames:
                        got = fn(t, sf(b), init) ^ xorout
                        want = int.from_bytes(b[6:8], 'big' if order == 'be' else 'little')
                        if got != want: ok = False; break
                    if ok:
                        print('CK MATCH:', mode, hex(poly), sname, 'init', hex(init),
                              'xor', hex(xorout), order)
