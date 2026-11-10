#!/usr/bin/env python3
"""Test CRC hypotheses at bytes[6:8] for each captured 0xAA frame.
Candidates computed over prefixes [0:L) for L in {12..60, and b1-based}.
Algorithms: CRC-16/MODBUS (poly A001 reflected), ~MODBUS, CCITT-FALSE, X25, XOR-fold."""
import json

recs = json.load(open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/ctrl9.json'))
frames = []
for r in recs:
    p = bytes.fromhex(r['payload']) if r['payload'] else b''
    if len(p) >= 12 and p[0] == 0xaa:
        frames.append(p)

def crc16_modbus(data):
    crc = 0xFFFF
    for b in data:
        crc ^= b
        for _ in range(8):
            crc = (crc >> 1) ^ 0xA001 if crc & 1 else crc >> 1
    return crc

def crc16_ccitt_false(data):
    crc = 0xFFFF
    for b in data:
        crc = ((crc ^ b) << 8) & 0xFFFF if True else 0
        for _ in range(8):
            crc = ((crc << 1) ^ 0x1021) & 0xFFFF if crc & 0x8000 else (crc << 1) & 0xFFFF
    return crc

def crc16_x25(data):
    crc = 0xFFFF
    for b in data:
        crc ^= b
        for _ in range(8):
            crc = (crc >> 1) ^ 0x8408 if crc & 1 else crc >> 1
    return crc ^ 0xFFFF

def crc16_add(data):  # additive/checksum style
    s = 0
    for b in data: s += b
    return s & 0xFFFF

hits = {}
for p in frames:
    target = p[6] << 8 | p[7]
    target_le = p[6] | p[7] << 8
    found = []
    for L in range(12, len(p) + 1):
        sub = p[:L]
        for name, fn in (('modbus', crc16_modbus), ('~modbus', lambda d: crc16_modbus(d) ^ 0xFFFF),
                         ('ccitt', crc16_ccitt_false), ('x25', crc16_x25), ('sum', crc16_add)):
            v = fn(sub)
            for endianness, tv in (('BE', target), ('LE', target_le)):
                if v == tv:
                    found.append((name, endianness, L))
        # also: CRC over [8:L) (skip first 8, CRC covers rest of header+payload)
        sub = p[8:L]
        for name, fn in (('modbus@8', crc16_modbus), ('~modbus@8', lambda d: crc16_modbus(d) ^ 0xFFFF),
                         ('ccitt@8', crc16_ccitt_false), ('x25@8', crc16_x25)):
            v = fn(sub)
            for endianness, tv in (('BE', target), ('LE', target_le)):
                if v == tv:
                    found.append((name, endianness, L))
    key = tuple(found[:3])
    hits.setdefault(key, []).append((p[1], p[2], p[9], p[10] << 8 | p[11], len(p)))

for k, v in hits.items():
    print(f"{k}  x{len(v)}")
    for row in v[:3]:
        print("    b1=%d seq=%d b9=%02x cmd=%04x len=%d" % row)
