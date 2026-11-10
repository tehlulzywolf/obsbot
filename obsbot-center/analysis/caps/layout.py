#!/usr/bin/env python3
"""Statistical layout analysis of captured 0xAA XU frames."""
import json
from collections import Counter

recs = json.load(open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/ctrl9.json'))
frames = []
for r in recs:
    p = bytes.fromhex(r['payload']) if r['payload'] else b''
    if len(p) >= 12 and p[0] == 0xaa:
        frames.append((r['bmt'], p))
print("0xAA frames:", len(frames), " dir OUT:", sum(1 for d,_ in frames if d == 0x21), " IN:", sum(1 for d,_ in frames if d == 0xa1))

def nz_end(p):
    n = len(p)
    while n > 0 and p[n-1] == 0: n -= 1
    return n

print("\nbyte[1] vs nonzero-end:")
for d, p in frames:
    print(f"  {'OUT' if d==0x21 else 'IN '} b1={p[1]:3d} b2={p[2]:3d} b3={p[3]:3d} b4b5={p[4]:02x}{p[5]:02x} b6b7={p[6]:02x}{p[7]:02x} b8={p[8]:02x} b9={p[9]:02x} b10b11={p[10]:02x}{p[11]:02x} nzend={nz_end(p):3d}")
