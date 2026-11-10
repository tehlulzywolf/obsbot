# Scan tx9.json for AI-path transactions: XU selector 6 (wV=0x0600) and any
# selector-2 AA frames with cmd pair 11 03 / 63 03 or frame type 03.
import json

tx = json.load(open("/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json"))
print("transactions:", len(tx))

def hx(b):
    return " ".join(f"{x:02x}" for x in bytes.fromhex(b)) if b else "-"

sel6 = 0
for t in tx:
    wv = t.get("wV", 0)
    out = t.get("out") or ""
    fr = bytes.fromhex(out) if out else b""
    pair3 = len(fr) >= 12 and fr[0] == 0xAA and (fr[10:12] in (bytes([0x11, 3]), bytes([0x63, 3])) or fr[1] == 3)
    if (wv >> 8) == 6 or pair3:
        sel6 += 1
        if sel6 <= 25:
            print(f"fr{t.get('fr')} wV={wv:04x} wI={t.get('wI'):04x} wL={t.get('wL')}")
            print("  out:", hx(out)[:160])
            print("  in :", hx(t.get("in"))[:160])
print("matched:", sel6)
