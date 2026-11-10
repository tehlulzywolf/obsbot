#!/usr/bin/env python3
"""Direct scan of cap1.pcap for AA vendor frames.
- OUT frames: pattern 'aa 01' at start of control OUT payload; SETUP = 8 bytes before.
- IN frames : pattern 'aa 29' inside completion payloads.
For every hit: pcap record offsets, SETUP bytes, full 60B payload.
Also: for each OUT frame record, find the completion record (same IRP id in d[0:8])
to reveal the response bytes."""
import struct, re, json
data = open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/cap1.pcap', 'rb').read()
print('file size', len(data))

# walk records
pos = 24
recs = []  # (rec_off, caplen, hdr16_off)
while pos + 16 <= len(data):
    t1, t2, cap, orig = struct.unpack_from('<IIII', data, pos)
    if cap > 0x10000:
        break
    recs.append((pos + 16, cap))
    pos += 16 + cap
print('records:', len(recs))

out_hits = []
in_hits = []
for i, (off, cap) in enumerate(recs):
    d = data[off:off + cap]
    if len(d) < 46 or d[14] != 9:
        continue
    j = d.find(b'\xaa\x01\x00', 30)
    while j != -1:
        # plausible frame start: next byte is seq-ish and payload covers 60
        if j + 60 <= cap:
            out_hits.append((i, off, j, bytes(d[j:j + 60]), bytes(d[max(0, j - 8):j])))
        j = d.find(b'\xaa\x01\x00', j + 1)
    k = d.find(b'\xaa\x29', 30)
    while k != -1:
        if k + 60 <= cap:
            in_hits.append((i, off, k, bytes(d[k:k + 60]), bytes(d[max(0, k - 8):k])))
        k = d.find(b'\xaa\x29', k + 1)
print('OUT aa0100 hits:', len(out_hits), ' IN aa29 hits:', len(in_hits))
seen = set()
uniq = []
for (i, off, j, p, pre) in out_hits:
    h = p[:6].hex()
    key = (j, p[:8].hex())
    if key in seen:
        continue
    seen.add(key)
    uniq.append((i, j, p, pre))
print('unique OUT frame instances:', len(uniq))
for (i, j, p, pre) in uniq[:10]:
    print(f'rec{i} pay@{j} pre={pre.hex(" ")}')
    print('   ', p[:20].hex(' '), '|', p[40:60].hex(' '))
print('--- IN samples ---')
for (i, off, k, p, pre) in in_hits[:6]:
    print(f'rec{i} pay@{k} pre={pre.hex(" ")}')
    print('   ', p[:20].hex(' '), '|', p[40:60].hex(' '))
json.dump({'out': [list(map(bytes.hex, (p, pre))) for (_, _, p, pre) in uniq],
           'in': [list(map(bytes.hex, (p, pre))) for (_, _, _, p, pre) in in_hits]},
          open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/aa_frames.json', 'w'))
print('saved aa_frames.json')
