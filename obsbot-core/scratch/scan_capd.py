# capE/capD USBPcap parser, self-syncing:
# header len u16@0 (0x1b/0x1c); SETUP located by scanning 24..40 for plausible
# control request (bmt/breq/wi), OUT data follows setup; AA frames at setup+8.
import struct, sys
from collections import Counter, defaultdict

def find_setup(d, caplen):
    for o in range(24, 41):
        if o + 8 > caplen: break
        bmt, breq = d[o], d[o + 1]
        if bmt in (0x21, 0xa1, 0x22, 0xa2) and breq in (0x01, 0x81, 0x85, 0x86):
            wi = d[o + 4] | (d[o + 5] << 8)
            if wi in (0x0000, 0x0100, 0x0200, 0x0300, 0x0400):
                return o
    return -1

def scan(path):
    ev = []
    with open(path, 'rb') as f:
        f.read(24)
        while True:
            ph = f.read(16)
            if len(ph) < 16: break
            ts_s, ts_u, caplen, _ = struct.unpack('<IIII', ph)
            d = f.read(caplen)
            if len(d) < caplen: break
            if caplen < 32: continue
            ts = ts_s + ts_u / 1e6
            o = find_setup(d, caplen)
            if o < 0: continue
            bmt, breq = d[o], d[o + 1]
            wv = d[o + 2] | (d[o + 3] << 8)
            wi = d[o + 4] | (d[o + 5] << 8)
            wl = d[o + 6] | (d[o + 7] << 8)
            sel = wv >> 8
            if bmt in (0x21, 0x22):  # SET (OUT)
                pay = d[o + 8:caplen]
                if sel == 2 and len(pay) >= 60 and pay[0] == 0xAA:
                    n = pay[12]
                    if n > 44: n = 44
                    ev.append((ts, 'AA-SET', f'ft={pay[1]:02x} fl={pay[9]:02x} seq={pay[2]:02x} pair={pay[10:12].hex()} n={n:02x} d={pay[16:16+n].hex()}'))
                else:
                    ev.append((ts, f'SET-s{sel}', f'wi={wi:04x} breq={breq:02x} len={len(pay)} d={pay[:120].hex()}'))
            else:  # GET (IN)
                ev.append((ts, f'GET-s{sel}', f'wi={wi:04x} breq={breq:02x} wl={wl}'))
    ev.sort(key=lambda x: x[0])
    return ev

paths = sys.argv[1:]
mode = 'dump'
if paths and paths[0] == '--hist':
    mode = 'hist'
    paths = paths[1:]
for path in paths:
    ev = scan(path)
    print(f'=== {path}  events={len(ev)}')
    kinds = Counter(e[1] for e in ev)
    print('kinds:', dict(kinds))
    if mode == 'hist':
        # per-kind payload-shape catalog
        shapes = defaultdict(lambda: Counter())
        for ts, k, desc in ev:
            if k.startswith('SET-s'):
                shapes[k][desc.split(' d=')[0]] += 1
                shapes[k + ':first16'][desc.split(' d=')[-1][:32]] += 1
        for k in sorted(shapes):
            print(f'--- {k}')
            for v, c in shapes[k].most_common(8):
                print(f'   {c:6d}  {v}')
    else:
        base = ev[0][0] if ev else 0
        for ts, k, desc in ev:
            print(f'{ts-base:8.2f} {k:9s} {desc}')
