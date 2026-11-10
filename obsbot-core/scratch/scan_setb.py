# Scan tx9 for SET_CUR AA frames that match the class-B shape from
# FUN_180100450: byte1 = small length value (0x0d..0x3c), byte3 has 0x10 bit.
import json

tx = json.load(open("/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json"))

def hx(b):
    return " ".join(f"{x:02x}" for x in bytes.fromhex(b)) if b else "-"

hits = 0
for t in tx:
    out = t.get("out") or ""
    if not out:
        continue
    fr = bytes.fromhex(out)
    if len(fr) < 12 or fr[0] != 0xAA:
        continue
    b1, b3 = fr[1], fr[3]
    clsB = 0x0c <= b1 <= 0x3c and (b3 & 0x10)
    # also flag anything unusual vs class A (ftype in {1,9,21,25,29,89})
    unusual = b1 not in (0x01, 0x09, 0x21, 0x25, 0x29, 0x89)
    if clsB or unusual:
        hits += 1
        if hits <= 20:
            print(f"fr{t.get('fr')} wV={t.get('wV'):04x} wI={t.get('wI'):04x} len={len(fr)}")
            print("  out:", hx(out)[:140])
print("unusual/classB SET frames:", hits, "of total SET frames")

# summary of ALL set frame byte1 values
from collections import Counter
c = Counter()
for t in tx:
    out = t.get("out") or ""
    if out:
        fr = bytes.fromhex(out)
        if fr and fr[0] == 0xAA:
            c[fr[1]] += 1
print("byte1 histogram:", {f"{k:02x}": v for k, v in sorted(c.items())})
