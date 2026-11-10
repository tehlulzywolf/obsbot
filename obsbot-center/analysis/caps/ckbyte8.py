#!/usr/bin/env python3
"""FUN_180163670: byte8 = T10[T10[...T10[T10[s0]^s1]^...^s11]] (12-byte chained XOR table).
Locate T10 (VA 0x1801cc960) in file, verify against ALL 0xAA frames in tx9.json."""
import json
data = open('/mnt/c/Program Files/OBSBOT Center/bin/libdev.dll', 'rb').read()

# parse sections for VA->file
e_lfanew = __import__('struct').unpack_from('<I', data, 0x3C)[0]
nsec = __import__('struct').unpack_from('<H', data, e_lfanew + 6)[0]
optsz = __import__('struct').unpack_from('<H', data, e_lfanew + 20)[0]
soff = e_lfanew + 24 + optsz
secs = []
for i in range(nsec):
    base = soff + i * 40
    va, vsz, raw, praw = (__import__('struct').unpack_from('<I', data, base + 12)[0],
                          __import__('struct').unpack_from('<I', data, base + 16)[0],
                          __import__('struct').unpack_from('<I', data, base + 20)[0],
                          __import__('struct').unpack_from('<I', data, base + 24)[0])
    secs.append((va, vsz, raw, praw))

def va2off(va):
    rva = va - 0x180000000
    for sva, svz, sraw, spr in secs:
        if sva <= rva < sva + max(svz, spr):
            return sraw + (rva - sva)
    raise KeyError(hex(va))

T10 = list(data[va2off(0x1801cc960):va2off(0x1801cc960) + 256])
assert T10[0] == 0x00 and T10[1] == 0x07 and T10[8] == 0x38, T10[:9]
print('T10 = CRC-8/0x07 table at file', hex(va2off(0x1801cc960)))

def f10(s):
    c = T10[s[0]]
    for b in s[1:12]:
        c = T10[c ^ b]
    return c

xu = json.load(open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json'))
ok = bad = 0
for r in xu:
    for hexs in (r['out'], r['in']):
        if len(hexs) == 120 and hexs[:2] == 'aa':
            w = bytes.fromhex(hexs)
            # build 12-byte pre-swap struct from wire (undo halfword swaps at 1,4,10)
            s = bytearray(w[0:12])
            s[1], s[2] = w[2], w[1]
            s[4], s[5] = w[5], w[4]
            s[10], s[11] = w[11], w[10]
            got = f10(s)
            # encoder writes byte8 = fn; then byte8 is overwritten post-CRC? check:
            if got == w[8]:
                ok += 1
            else:
                bad += 1
                if bad <= 6:
                    print('NO', w[:12].hex(' '), '->', hex(got), 'wire8', hex(w[8]))
print(f'byte8 model verified {ok}/{ok + bad}')
