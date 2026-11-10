#!/usr/bin/env python3
"""Determine the byte9 relationship, then brute ck over prefixes."""
REQ = [
 'aa0100000c00915c0a0d0818000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000',
 'aa0101000c00c1500a0d080400000000000037be8bbcbd72000000000000000000000000000000000000000000000000000000000000000000000000',
 'aa0102000c00c0960a0d4819000000000000000000000000000037be8bbcbd7201000000000000000000000000000000000000000000000000000000',
 'aa0103000c0031530a0dc818000000000000000000000000000037be8bbcbd7201000000000000000000000000000000000000000000000000000000',
 'aa0104000c00d6730a0242c4000000007000000000000000000027cb5c02000007000000000000000000000019000200600000000000000044210dc0',
 'aa0105000c0095840a040401000000000000000000000000000037be8bbcbd7201000000000000000000000000000000000000000000000000000000',
]
RESP = [
 'aa2900000c00a0490d0a0818180059fc000000000000000000000000000000000000000000000000000037be8bbcbd72000000000000000000000000',
 'aa2901000c00f0450d0a08040800540c010602060000000000000000000000000000000000000000000037be8bbcbd72010000000000000000000000',
 'aa2902000c00f1830d0a48192200987f080008020106020600000000000000000000000000000000000037be8bbcbd72010000000000000000000000',
 'aa2903000c0000460d0ac8180e0052f1524d4f574c4847423038314a494c000000000000000000000000000000000000000000000000000000000000',
 'aa890400100046b7020a42c40002fdff503302020106020600000000000000000000000000000000000037be8bbcbd72010000000000000000000000',
]
FR = [bytes.fromhex(h) for h in REQ + RESP]

# 1) byte9 vs cmd byte11
for b in FR:
    print(f"b8={b[8]:02x} b9={b[9]:02x} b10={b[10]:02x} b11={b[11]:02x} b1={b[1]:02x} ck={b[6:8].hex()}")
# 2) candidate: ck = crc16 over b[0:6] + b[8:60] (excl ck) and prefixes
def crc16_tbl(data, poly, init, xorout, reflect=True):
    def rf(x):
        r = 0
        for i in range(16):
            if x >> i & 1: r |= 1 << (15 - i)
        return r
    crc = init
    for by in data:
        if reflect: by = int('{:08b}'.format(by)[::-1], 2)
        crc ^= by << 8
        for _ in range(8):
            crc = ((crc << 1) ^ poly) & 0xFFFF if crc & 0x8000 else (crc << 1) & 0xFFFF
    if reflect: crc = rf(crc)
    return crc ^ xorout
polys = []
for name, poly, init, ref, xorout in [
    ('MODBUS', 0x8005, 0xFFFF, True, 0), ('CCITT', 0x1021, 0xFFFF, False, 0),
    ('X25', 0x8005, 0xFFFF, True, 0xFFFF), ('BOTH', 0x8005, 0xFFFF, True, 0xFFFF),
    ('GENIBUS', 0x1021, 0xFFFF, False, 0xFFFF), ('AUG', 0x1021, 0xC6C6, True, 0),
    ('KERMIT', 0x1021, 0, True, 0), ('CRCCITT0', 0x1021, 0, False, 0)]:
    for span, sf in [('0:6+8:', lambda x: x[0:6] + x[8:60]), ('8:60', lambda x: x[8:60]),
                     ('0:6', lambda x: x[0:6]), ('2:6+8:', lambda x: x[2:6] + x[8:60]),
                     ('10:60', lambda x: x[10:60]), ('12:60', lambda x: x[12:60]),
                     ('11:60', lambda x: x[11:60]), ('9:60', lambda x: x[9:60])]:
        vals = set()
        for b in FR:
            got = crc16_tbl(sf(b), poly, init, xorout, ref)
            vals.add(got.to_bytes(2, 'big') == b[6:8] or got.to_bytes(2, 'little') == b[6:8])
        if all(vals): print('CRC MATCH:', name, span)
# 3) byte-wise: maybe ck = f(seq): seq 0..5 ->
print('seq vs ck BE:')
for b in FR:
    print(' ', b[2], b[6:8].hex(), 'b1=', hex(b[1]))
