# Build request->reply pair catalog from tx9: every SET_CUR/GET_CUR on wV=0200
# where OUT or IN is an AA frame. Report (seq, pair, ftype, payload) tuples and
# which pairs Rust already sends.
import json

tx = json.load(open("/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json"))

reqs, reps = {}, []
for t in tx:
    out = t.get("out") or ""
    inn = t.get("in") or ""
    if out:
        fr = bytes.fromhex(out)
        if len(fr) >= 16 and fr[0] == 0xAA:
            pair = fr[10:12].hex()
            seq = fr[2] | (fr[3] << 8)
            n = fr[12]
            reqs.setdefault(seq, []).append(
                (t["fr"], pair, fr[1], fr[9], fr[12], bytes(fr[16:16+n]).hex()))
    if inn:
        ir = bytes.fromhex(inn)
        if len(ir) >= 16 and ir[0] == 0xAA:
            seq = ir[2] | (ir[3] << 8)
            n = ir[12]
            reps.append((t["fr"], seq, ir[10:12].hex(), ir[1], ir[9], n, bytes(ir[16:16+n]).hex()))

seen = set()
print("=== replies (pair -> decoded payload) ===")
for fr, seq, pair, ft, fl, n, pl in reps:
    key = (pair, pl)
    if key in seen:
        continue
    seen.add(key)
    req = next((r for r in reqs.get(seq, []) if r[1] == pair), None)
    reqs_str = f"req ftype={req[2]:02x} flags={req[3]:02x} args={req[5]}" if req else "req=? (pre-capture)"
    print(f"fr{fr} seq={seq:2d} pair={pair} rep_ft={ft:02x} len={n:2d} pl={pl}  | {reqs_str}")
