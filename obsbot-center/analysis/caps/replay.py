#!/usr/bin/env python3
"""Live replay test on Tiny 2 Lite (WSL):
send captured XU frames EXACTLY as the Windows app did, read replies.
Steps mirror cap1.pcap ground truth:
  GET LEN (a1 01 wV=0200 wI=0000 wL=2)  -> expect 3c00
  SET CUR (21 01 wV=0200 wI=0000 wL=60 payload=aa-frame)
  GET CUR (a1 01 wV=0200 wI=0000 wL=60) -> expect aa29.. reply
Then EP 0x86 interrupt stream peek."""
import sys, time
import usb.core, usb.util

dev = usb.core.find(idVendor=0x3564, idProduct=0xfef9)
if dev is None:
    sys.exit('NO DEVICE')
print('device found', dev.idVendor, dev.idProduct)
try:
    if dev.is_kernel_driver_active(0):
        dev.detach_kernel_driver(0)
        print('detached uvcvideo from iface 0')
except Exception as ex:
    print('detach:', ex)
dev.set_configuration()
cfg = dev.get_active_configuration()
print('config set, cfg', cfg.bConfigurationValue)

# find EP 0x86 interrupt IN
ep_in = None
for intf in cfg:
    for ep in intf:
        if ep.bEndpointAddress == 0x86:
            ep_in = ep
print('EP86 found:', ep_in is not None, 'size', getattr(ep_in, 'wMaxPacketSize', None))

TIMEOUT = 2000

def step(tag, bm, breq, wv, wi, data=None, wlen=0):
    try:
        if bm & 0x80:
            r = dev.ctrl_transfer(bm, breq, wv, wi, wlen, timeout=TIMEOUT)
            print(f'{tag}: IN  -> {bytes(r).hex(" ")}')
            return bytes(r)
        else:
            n = dev.ctrl_transfer(bm, breq, wv, wi, data, timeout=TIMEOUT)
            print(f'{tag}: OUT sent {n} bytes')
            return n
    except usb.core.USBError as ex:
        print(f'{tag}: ERROR {ex}')
        return None

# 1. GET LEN for selector 2
step('GETLEN sel2', 0xA1, 0x01, 0x0200, 0x0000, wlen=2)
# 2. SET CUR with captured req0 frame (cmd 0818)
import json
xu = json.load(open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json'))
req = None
for r in xu:
    if r['bmt'] == 0x21 and len(r['out']) == 120 and r['out'][:2] == 'aa':
        req = bytes.fromhex(r['out'])
        break
print('replaying frame:', req[:12].hex(' '))
step('SETCUR', 0x21, 0x01, 0x0200, 0x0000, data=req)
time.sleep(0.15)
r = step('GETCUR', 0xA1, 0x01, 0x0200, 0x0000, wlen=60)
if r and r[:1] == b'\xaa':
    print('>>> REPLY OK:', r[:20].hex(' '))

# 3. wrong-checksum variant to test device validation
bad = bytearray(req)
bad[6] ^= 0xFF
bad[7] ^= 0xFF
step('SETCUR-badck', 0x21, 0x01, 0x0200, 0x0000, data=bytes(bad))
time.sleep(0.15)
r2 = step('GETCUR-badck', 0xA1, 0x01, 0x0200, 0x0000, wlen=60)
print('bad-ck reply:', (r2[:12].hex(' ') if r2 else None))

# 4. EP6 event stream
if ep_in:
    try:
        ep_in = usb.util.find_descriptor(
            cfg[(0, 0)] if False else cfg.interfaces(0)[0] if False else cfg[0],
            custom_match=lambda e: e.bEndpointAddress == 0x86) or ep_in
        for _ in range(4):
            d = ep_in.read(64, timeout=1500)
            print('EP6:', bytes(d).hex(' '))
    except usb.core.USBError as ex:
        print('EP6:', ex)
