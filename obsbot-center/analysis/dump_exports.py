# Dump PE export table of libdev.dll (names + RVAs) using stdlib only
import struct, sys, json, csv

path = r"C:\Program Files\OBSBOT Center\bin\libdev.dll"
data = open(path, "rb").read()

def off(rva):
    # map RVA -> file offset via section headers
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    nsec = struct.unpack_from("<H", data, pe+6)[0]
    optsize = struct.unpack_from("<H", data, pe+20)[0]
    secs = []
    for i in range(nsec):
        s = pe + 24 + optsize + i*40
        name = data[s:s+8].rstrip(b"\0").decode()
        va, vsz, raw, rawoff = struct.unpack_from("<IIII", data, s+12)[0], struct.unpack_from("<I",data,s+8)[0], struct.unpack_from("<I",data,s+16)[0], struct.unpack_from("<I",data,s+20)[0]
        va2, raw2 = struct.unpack_from("<I", data, s+12)[0], struct.unpack_from("<I", data, s+20)[0]
        vsz2 = struct.unpack_from("<I", data, s+8)[0]
        secs.append((va2, max(vsz2,0), raw2, data[s:s+8].rstrip(b'\0').decode()))
    def f(r):
        for va, vsz, rawoff, nm in secs:
            if va <= r < va+max(vsz,0x1000):
                return rawoff + (r - va)
        raise ValueError(hex(r))
    return f(rva)

pe = struct.unpack_from("<I", data, 0x3C)[0]
opt = pe + 24
magic = struct.unpack_from("<H", data, opt)[0]
ddoff = opt + (112 if magic == 0x10B else 112)  # magic 0x20B: 112; 0x10B: 96
ddoff = opt + (96 if magic == 0x10B else 112)
exp_rva, exp_sz = struct.unpack_from("<II", data, ddoff + 8*0)
f = off(exp_rva)
flags, tstamp, maj, min_ = struct.unpack_from("<2I2H", data, f)
fname_rva, ordbase, nfunc, nname, funcs_rva, names_rva, ords_rva = struct.unpack_from("<7I", data, f+12)
ff = off(funcs_rva); fn = off(names_rva); fo = off(ords_rva)
fname_rva = struct.unpack_from("<I", data, f+12)[0]
exports = []
for i in range(nname):
    nrva = struct.unpack_from("<I", data, fn + i*4)[0]
    o = struct.unpack_from("<H", data, fo + i*2)[0]
    frva = struct.unpack_from("<I", data, ff + o*4)[0]
    end = data.index(b"\0", off(nrva))
    nm = data[off(nrva):end].decode("ascii", "replace")
    exports.append((o+ordbase, frva, nm))
print("count:", len(exports))
out = r"C:\Users\WOLF\RE-workspace\obsbot-center\analysis\libdev_exports.csv"
with open(out, "w", newline="") as fh:
    w = csv.writer(fh); w.writerow(["ordinal","rva","name"])
    for e in sorted(exports): w.writerow(e)
print("wrote", out)
# quick filter for control-relevant names
import re
keys = ["gimbal","pan","tilt","preset","track","zoom","calib","move","rotat","ctrl","standby","sleep","power","reset","updat","face","human","frame","led","mic"]
hit = [e for e in exports if any(k in e[2].lower() for k in keys)]
print("control-relevant:", len(hit))
for e in hit[:120]: print(hex(e[1]), e[2])
