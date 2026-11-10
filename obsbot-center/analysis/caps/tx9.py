#!/usr/bin/env python3
"""Ground-truth transaction parser for cap1.pcap (dev 9).
Empirical header (DLT_USBPCAP, 32-byte base):
  0  u16 hdrlen (0x1b=27 submit-with-no-data? 0x1c=28) — actually varies
  8  u64 irp_id (paired submit<->complete)
  12 u32 function (1=submit,2=complete)
  16 u8 endpoint/flags  19 u8 device  20..24 misc (00 01 00)
  control submit: setup = [28:36], out-data = [36:cap]
  complete IN:    data = [32:cap]   (cap-32 = actual length)
Output JSON: transactions {setup, out_data, in_data}."""
import struct, json
from collections import OrderedDict

PCAP = '/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/cap1.pcap'
tx = OrderedDict()   # irp -> dict
iso = []             # EP6-ish IN event payloads
with open(PCAP, 'rb') as f:
    f.read(24)
    n = 0
    while True:
        ph = f.read(16)
        if len(ph) < 16: break
        ts_s, ts_u, caplen, _ = struct.unpack('<IIII', ph)
        d = f.read(caplen)
        if len(d) < caplen: break
        n += 1
        if caplen < 28: continue
        irp = struct.unpack('<Q', d[0:8])[0]
        fn = struct.unpack('<I', d[12:16])[0]
        ep = d[16]
        dev = d[23]
        if dev != 9: continue
        t = d[24:28]
        if ep in (0x81, 0x02, 0x82, 0x83, 0x03) and fn in (1, 2):
            pass  # bulk/isoc handled below by ep pattern
        if fn == 1 and d[24] == 0x00 and d[25] == 0x01 and caplen >= 36 and (d[16] & 0x07) == 3:
            # control submit: t[0]=00? detect by setup presence: bmRequestType sanity
            setup = d[28:36]
            bmt, breq = setup[0], setup[1]
            if bmt in (0x21, 0xa1, 0x23, 0x41, 0x80, 0x00, 0x22, 0x81, 0xa0):
                e = tx.setdefault(irp, {})
                e['fr'] = n; e['t'] = ts_s + ts_u / 1e6
                e['setup'] = setup.hex()
                if caplen > 36: e['out'] = d[36:caplen].hex()
                continue
        if fn == 2 and caplen > 32:
            # completion carrying IN data
            bmt_guess = None
            e = tx.get(irp)
            if e is not None:
                e['fr2'] = n
                e['in'] = d[32:caplen].hex()
            else:
                iso.append({'fr': n, 't': round(ts_s + ts_u / 1e6, 6), 'ep': hex(d[16]),
                            'data': d[32:caplen].hex()[:200]})

print('control transactions:', len(tx), ' bulk/isoc IN payloads:', len(iso))
res = []
for irp, e in tx.items():
    s = bytes.fromhex(e['setup'])
    wValue = s[2] | s[3] << 8; wIndex = s[4] | s[5] << 8; wLen = s[6] | s[7] << 8
    res.append({'fr': e['fr'], 't': round(e['t'], 4), 'bmt': s[0], 'breq': s[1],
                'wV': wValue, 'wI': wIndex, 'wL': wLen,
                'out': e.get('out', ''), 'in': e.get('in', ''), 'reply': 'in' in e})
json.dump(res, open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json', 'w'))
json.dump(iso, open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/ep_in.json', 'w'))

replies = [r for r in res if r['in'] and r['breq'] in (0x01, 0x81, 0x85)]
print('replied:', len(replies))
for r in replies[:40]:
    tag = {1: 'SET', 0x81: 'GET', 0x85: 'LEN'}.get(r['breq'], hex(r['breq']))
    print(f"f{r['fr']} {tag} wV={r['wV']:#06x} wI={r['wI']:#06x} wL={r['wL']} inlen={len(r['in'])//2}")
    if r['in']: print('   IN :', r['in'][:110])
    if r['out'] and tag == 'SET': print('   OUT:', r['out'][:110])
