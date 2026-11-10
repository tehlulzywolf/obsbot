#!/usr/bin/env python3
"""Parse ctrl9_x.txt: tshark -x dumps of dev9 control URBs.
Each block: 'NNNN  <frame hex>' lines; USBPCAP URB header is first 24 bytes,
then for control: 8-byte SETUP, then optional data stage bytes (merged) or
separate 'USB Control' section. We just scan frame bytes for setup signatures."""
import re, sys, json

txt = open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/ctrl9_x.txt').read()
# split into frame blocks: lines '0000  xx xx ...' start after a 'Packet (N bytes):' or directly frame-number line
blocks = []
cur = []
frameno = None
for line in txt.splitlines():
    m = re.match(r'^\s*(\d{4})\s', line)
    if m and int(m.group(1)) == 0 and cur:
        blocks.append((frameno, cur)); cur = []
    if re.match(r'^\s{4}\d', line):  # 'Packet (96 bytes):' marker line has 4 leading spaces? tshark -x prints "Packet (N bytes):" at col0 sometimes
        pass
    cur.append(line)
blocks.append((frameno, cur))

# simpler robust approach: treat consecutive hex lines; a new frame begins when offset resets to 0000
frames = []
hexlines = {}
for chunk in re.split(r'\n(?=0000  )', txt):
    data = bytearray()
    for ln in chunk.splitlines():
        m = re.match(r'^([0-9a-f]{4})\s+(..(?: ..){0,15})', ln.strip())
        if m:
            hexstr = m.group(2).replace(' ', '')
            # ascii part after two spaces may be captured; re-parse: take only hex pairs
            parts = ln.strip().split()
            hx = []
            for p in parts[1:]:
                if re.fullmatch(r'[0-9a-f]{2}', p): hx.append(p)
                else: break
            data += bytes.fromhex(''.join(hx))
    if data:
        frames.append(data)

print("frames parsed:", len(frames))
results = []
for d in frames:
    if len(d) < 24 + 8: continue
    urb = d[:24]
    setup = d[24:32]
    payload = d[32:]
    bmt, breq, wvalue, windex, wlen = setup[0], setup[1], setup[2] | setup[3] << 8, setup[4] | setup[5] << 8, setup[6] | setup[7] << 8
    results.append({'bmt': bmt, 'breq': breq, 'wValue': wvalue, 'wIndex': windex, 'wLen': wlen,
                    'payload': payload.hex(), 'plen': len(payload)})

from collections import Counter
c = Counter((r['bmt'], r['breq'], r['wValue'], r['wIndex'], r['wLen']) for r in results)
for k, n in c.most_common(15):
    print(f"bmt={k[0]:#04x} breq={k[1]:#04x} wValue={k[2]:#06x} wIndex={k[3]:#06x} wLen={k[4]} x{n}")

print()
print("=== SET_CUR OUT payloads (dir=host->dev, bmt=0x21/0x41/0x11 style class/video) ===")
for r in results:
    if r['breq'] == 0x01 and r['plen'] > 0:
        print(f"wV={r['wValue']:#06x} wI={r['wIndex']:#06x} wL={r['wLen']} plen={r['plen']}")
        print("   ", r['payload'][:80])

json.dump(results, open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/ctrl9.json', 'w'))
print("saved ctrl9.json")
