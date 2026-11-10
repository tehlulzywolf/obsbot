#!/usr/bin/env python3
"""Parse ctrl9_x.txt: split on 'Packet (N bytes):' sections, take hex lines,
strip 24-byte USBPCAP URB header, decode 8-byte SETUP + payload."""
import re, json

txt = open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/ctrl9_x.txt', encoding='utf-8', errors='replace').read()
sections = []
for block in txt.split('\n\n'):
    data = bytearray()
    for ln in block.splitlines():
        m = re.match(r'^([0-9a-f]{4})\s\s((?:[0-9a-f]{2}\s)+)', ln)
        if m:
            data += bytes.fromhex(m.group(2).replace(' ', ''))
        elif data:
            break  # end of first contiguous raw dump in this block
    if data:
        sections.append(data)

print("packet sections:", len(sections))
recs = []
for d in sections:
    if len(d) < 32: continue
    setup = d[28:36]
    payload = d[36:]
    bmt, breq = setup[0], setup[1]
    wValue = setup[2] | setup[3] << 8
    wIndex = setup[4] | setup[5] << 8
    wLen = setup[6] | setup[7] << 8
    recs.append(dict(bmt=bmt, breq=breq, wValue=wValue, wIndex=wIndex, wLen=wLen, payload=payload.hex()))

# summary of XU (class, interface) requests
from collections import Counter
c = Counter((r['bmt'], r['breq'], r['wValue'], r['wIndex'], r['wLen']) for r in recs)
print("=== request tuples (bmt breq wValue wIndex wLen) x count ===")
for k, n in c.most_common(12):
    print(f"  bmt={k[0]:#04x} breq={k[1]:#04x} wValue={k[2]:#06x} wIndex={k[3]:#06x} wLen={k[4]}  x{n}")

print("=== OUT class SET_CUR payloads (wLen=60) ===")
n = 0
for r in recs:
    if r['bmt'] == 0x21 and r['breq'] == 0x01 and len(r['payload']) >= 24:
        print(f"  wV={r['wValue']:#06x} wI={r['wIndex']:#06x}: {r['payload'][:64]}")
        n += 1
        if n >= 12: break
print("=== IN class GET_CUR responses ===")
n = 0
for r in recs:
    if r['bmt'] == 0xa1 and r['breq'] == 0x01 and len(r['payload']) >= 12:
        print(f"  wV={r['wValue']:#06x} wI={r['wIndex']:#06x} len={len(r['payload'])//2}: {r['payload'][:80]}")
        n += 1
        if n >= 12: break
json.dump(recs, open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/ctrl9.json', 'w'))
print("saved", len(recs), "records")
