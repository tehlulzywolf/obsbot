
import struct, json, sys
path = r"C:/Program Files/OBSBOT Center/bin/libdev.dll"
data = open(path, "rb").read()
# parse PE headers
e_lfanew = struct.unpack_from("<I", data, 0x3C)[0]
assert data[e_lfanew:e_lfanew+4] == b"PE\0\0"
coff = e_lfanew + 4
num_sec, = struct.unpack_from("<H", data, coff+2)
opt_size, = struct.unpack_from("<H", data, coff+16)
opt = coff + 20
image_base, = struct.unpack_from("<Q", data, opt+0x18)
sec = opt + opt_size
sections = []
for i in range(num_sec):
    o = sec + i*40
    name = data[o:o+8].rstrip(b"\0").decode()
    vsize, va, rawsize, rawptr = struct.unpack_from("<IIII", data, o+8)
    sections.append((name, image_base+va, max(vsize, rawsize), rawsize, rawptr))

def va2off(va):
    for name, sva, cover, rawsize, rawptr in sections:
        if sva <= va < sva + cover and va - sva < rawsize:
            return rawptr + (va - sva)
    for s in sections:
        print("SEC", s[0], hex(s[1]), "end", hex(s[1]+s[2]))
    raise KeyError(hex(va))

va = 0x1801cca60
off = va2off(va)
t1 = data[off:off+256]
t2va = 0x1801ccb60
off2 = va2off(t2va)
t2 = data[off2:off2+256]
# sanity: CRC tables usually start 00 00 / table-ish; print heads
print("sections:", [(s[0], hex(s[1])) for s in sections])
print("off", hex(off), "off2", hex(off2), "gap", hex(t2va-va))
print("T1[:16]:", t1[:16].hex(" "))
print("T2[:16]:", t2[:16].hex(" "))

# Try to model the loop: b6 = crc_lo?, b4 = crc_hi?
# loop: idx = b6 ^ byte ; b6 = b4 ^ T1[idx] ; b4 = T2[idx]
def crc(frames, t1, t2):
    b4, b6 = 0xff, 0xff
    for byte in frames:
        idx = b6 ^ byte
        b6 = (b4 ^ t1[idx]) & 0xff
        b4 = t2[idx]
    return (~((b4<<8)|b6)) & 0xffff

# Test against classic CRC-16/CRUSHFA? compare with known algos using these tables.
# Standard CRC-16/XMODEM check "123456789" = 0x3153 ; CRC-16/KERMIT = 0x2189; CRC-16/CRC-CCITT-FALSE=0x3994; DNP?
test = b"123456789"
print("custom table crc:", hex(crc(test, t1, t2)))

open(r"C:/Users/WOLF/RE-workspace/obsbot-center/analysis/crc_tables.json","w").write(json.dumps({
  "va_t1": hex(va), "va_t2": hex(t2va),
  "t1": list(t1), "t2": list(t2),
}))
print("saved crc_tables.json")
