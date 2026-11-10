#!/usr/bin/env python3
"""CRC-16 family sweep on 11 verified request frames (ck @ [6:8]).
Try: span variants x reflected/non-reflected x init x xorout.
Also test f4 05 26 02 / seed-rotation theories."""
import json
xu = json.load(open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json'))
REQS = []
for r in xu:
    if r['bmt'] == 0x21 and len(r['out']) == 120 and r['out'][:2] == 'aa':
        REQS.append(bytes.fromhex(r['out']))
REQS = REQS[:11]
print('reqs:', len(REQS))
for b in REQS:
    print(' ', b[:12].hex(' '), '...', b[56:60].hex(' '))

def r8(x):
    r = 0
    for i in range(8):
        if x >> i & 1: r |= 1 << (7 - i)
    return r

def crc16(buf, poly, init, refin, xorout):
    crc = init
    for by in buf:
        if refin: by = r8(by)
        crc ^= by << 8
        for _ in range(8):
            crc = ((crc << 1) ^ poly) & 0xFFFF if crc & 0x8000 else (crc << 1) & 0xFFFF
    return crc ^ xorout

def crc16r(buf, poly, init, xorout):
    """fully-reflected (LSB-first) table-style"""
    crc = init
    pr = int('{:016b}'.format(poly)[::-1], 2)
    for by in buf:
        crc = (crc >> 8) ^ 0  # placeholder
        crc ^= by
        for _ in range(8):
            crc = (crc >> 1) ^ pr if crc & 1 else crc >> 1
    return crc ^ xorout

spans = {
    '0:6+8:60': lambda b: b[0:6] + b[8:60],
    'full60':   lambda b: b[0:60],
    'zeroCk':   lambda b: b[0:6] + b'\0\0' + b[8:60],
    '0:2+8:60': lambda b: b[0:2] + b[8:60],
    '2:6+8:60': lambda b: b[2:6] + b[8:60],
    '8:60':     lambda b: b[8:60],
    '10:60':    lambda b: b[10:60],
    '12:60':    lambda b: b[12:60],
    '9:60':     lambda b: b[9:60],
    '0:6':      lambda b: b[0:6],
    '0:12z':    lambda b: b[0:6] + b'\0\0' + b[8:12],
    '0:12':     lambda b: b[0:12],
    '6:60':     lambda b: b[6:60],   # ck included running-crc style
}
polys = {
    'MODBUS/8005r': (0xA001, True), '8005': (0x8005, False),
    '1021': (0x1021, False), '1021r(8408)': (0x8408, True),
    '1021/0x0520?': (0x0520, True), '3D65': (0x3D65, False),
    '8BB7(Kermit)': (0x1021, False), '1D0F(CCITT seed)': (0x1D0F, False),
    '8005/0xC6C6': (0xC6C6, True), 'f4052602 hi/lo?': (0xF405, False),
    '2602': (0x2602, False), '17(USB)': (0x8005, False),
}
results = []
for pname, (poly, refl) in polys.items():
    for sname, sf in spans.items():
        for init in (0x0000, 0xFFFF, 0x1D0F, 0x8408, 0x020A, 0x0A0D, 0x0C00, 0x0001):
            for xorout in (0x0000, 0xFFFF):
                for order in ('BE', 'LE'):
                    ok = True
                    for b in REQS:
                        want = int.from_bytes(b[6:8], 'big' if order == 'BE' else 'little')
                        data = sf(b)
                        got = crc16r(data, poly if not refl else poly, init, xorout) if refl else crc16(data, poly, init, False, xorout)
                        if got != want:
                            ok = False; break
                    if ok:
                        results.append((pname, sname, hex(init), hex(xorout), order))
                        print('HIT:', pname, sname, hex(init), hex(xorout), order)
print('hits:', results if results else 'NONE')
