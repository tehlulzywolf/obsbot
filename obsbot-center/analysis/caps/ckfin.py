#!/usr/bin/env python3
"""Extract the two CRC tables used by FUN_1800ffce0 (DAT_1801cca60 / DAT_1801ccb60)
by converting VA -> file offset via the PE headers, then verify the encoder
against all captured frames."""
import struct, json

DLL = '/mnt/c/Program Files/OBSBOT Center/bin/libdev.dll'
data = open(DLL, 'rb').read()

# PE headers
e_lfanew = struct.unpack_from('<I', data, 0x3C)[0]
nsec = struct.unpack_from('<H', data, e_lfanew + 6)[0]
optsz = struct.unpack_from('<H', data, e_lfanew + 20)[0]
sect = e_lfanew + 24 + optsz
secs = []
for i in range(nsec):
    name = data[sect + i * 40:sect + i * 40 + 8].rstrip(b'\0').decode()
    va, vsz, raw, praw = struct.unpack_from('<IIII', data, sect + i * 40 + 12)[0:4]
    va = struct.unpack_from('<I', data, sect + i * 40 + 12)[0]
    vsz = struct.unpack_from('<I', data, sect + i * 40 + 16)[0]
    raw = struct.unpack_from('<I', data, sect + i * 40 + 20)[0]
    praw = struct.unpack_from('<I', data, sect + i * 40 + 24)[0]
    secs.append((name, va, vsz, raw, praw))
    print(name, hex(va), hex(vsz), hex(raw), hex(praw))

imgbase = 0x180000000
def va2off(va):
    rva = va - imgbase
    for name, sva, svz, sraw, spraw in secs:
        if sva <= rva < sva + max(svz, spraw):
            return sraw + (rva - sva)
    raise KeyError(hex(va))

VA_T2 = 0x1801CCA60   # used as bVar6 = bVar4 ^ T2[idx]   (low half)
VA_T1 = 0x1801CCB60   # bVar4 = T1[idx]                    (high half)
off2, off1 = va2off(VA_T2), va2off(VA_T1)
print('T2 @', hex(off2), 'T1 @', hex(off1), 'gap', hex(VA_T2 - VA_T1))
T2 = list(data[off2:off2 + 256])
T1 = list(data[off1:off1 + 256])
print('T2[:8]', T2[:8])
print('T1[:8]', T1[:8])

def ck_calc(buf, length):
    b4 = b6 = 0xFF
    for i in range(length):
        idx = b6 ^ buf[i]
        b6 = b4 ^ T2[idx]
        b4 = T1[idx]
    return (~((b4 << 8) | b6)) & 0xFFFF

xu = json.load(open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json'))
FR = []
for r in xu:
    for hexs in (r['out'], r['in']):
        if len(hexs) == 120 and hexs[:2] == 'aa':
            FR.append(bytes.fromhex(hexs))
ok = 0
bad = []
for b in FR:
    # length field at [1:3] as encoded (BE halfword b1b2): L = u16 & 0xFFF
    L = ((b[1] << 8) | b[2]) & 0xFFF
    if L == 0 or L > 60:
        L = 12  # requests/len12
    sim = bytearray(b)
    want = int.from_bytes(b[6:8], 'big')
    sim[6] = sim[7] = 0
    sim[8] = 0
    got = ck_calc(sim, L)
    if got == want:
        ok += 1
    else:
        bad.append((b[:12].hex(' '), f'L={L} want={want:04x} got={got:04x}'))
print(f'verified {ok}/{len(FR)}')
for x in bad[:8]:
    print('BAD', x)
