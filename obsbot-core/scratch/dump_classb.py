# Dump all class-B (0x21/0x25) frames from tx9 with full hex, in/out.
import json

tx = json.load(open("/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json"))
for t in tx:
    out = t.get("out") or ""
    inn = t.get("in") or ""
    fr = bytes.fromhex(out) if out else b""
    ir = bytes.fromhex(inn) if inn else b""
    interesting = (
        (fr and fr[0] == 0xAA and fr[1] in (0x21, 0x25))
        or (ir and ir[0] == 0xAA and ir[1] in (0x21, 0x25, 0x29))
    )
    if interesting:
        print(f"fr{t.get('fr')} wV={t.get('wV'):04x} wI={t.get('wI'):04x} wL={t.get('wL')}")
        if out:
            print("  OUT:", " ".join(f"{x:02x}" for x in fr))
        if inn:
            print("  IN :", " ".join(f"{x:02x}" for x in ir))
        print()
