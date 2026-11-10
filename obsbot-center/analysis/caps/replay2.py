#!/usr/bin/env python3
"""Live replay v2 on Tiny 2 Lite (WSL), EXACTLY mirroring cap1.pcap ground truth:
  GET_LEN  a1 85 wV=0200 wI=0200 wL=2   -> expect 3c 00
  SET_CUR  21 01 wV=0200 wI=0200 wL=60 payload=aa-frame   (send ALL captured in order)
  GET_CUR  a1 81 wV=0200 wI=0200 wL=60  -> expect aa 29 ..
Then EP 0x86 interrupt peek, then bad-checksum test."""
import sys, time, json
import usb.core, usb.util

dev = usb.core.find(idVendor=0x3564, idProduct=0xfef9)
if dev is None:
    sys.exit('NO DEVICE')
print('device found')
try:
    for i in (0, 1, 2, 3):
        try:
            if dev.is_kernel_driver_active(i):
                dev.detach_kernel_driver(i)
                print('detached iface', i)
        except Exception:
            pass
except Exception as ex:
    print('detach:', ex)
try:
    dev.set_configuration()
    print('config set ->', dev.get_active_configuration().bConfigurationValue)
except usb.core.USBError as ex:
    print('set_config:', ex)
cfg = dev.get_active_configuration()

TIMEOUT = 2000

def step(tag, bm, breq, wv, wi, data=None, wlen=0):
    try:
        if bm & 0x80:
            r = bytes(dev.ctrl_transfer(bm, breq, wv, wi, wlen, timeout=TIMEOUT))
            print(f'{tag}: IN  -> {r.hex(" ")}')
            return r
        n = dev.ctrl_transfer(bm, breq, wv, wi, data, timeout=TIMEOUT)
        print(f'{tag}: OUT sent {n}B')
        return n
    except usb.core.USBError as ex:
        print(f'{tag}: ERROR {ex}')
        return None

# 1. GET_LEN first (capture shows it precedes every SET/GET CUR)
step('GETLEN sel2', 0xA1, 0x85, 0x0200, 0x0200, wlen=2)

xu = json.load(open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json'))
seq = [bytes.fromhex(r['out']) for r in xu
       if r['bmt'] == 0x21 and r['wV'] == 0x0200 and len(r['out']) == 120 and r['out'][:2] == 'aa']
print('captured SET frames to replay:', len(seq))
for i, f in enumerate(seq):
    step(f'SET[{i}] cmd={f[10:12].hex(" ")}', 0x21, 0x01, 0x0200, 0x0200, data=f)
    time.sleep(0.08)

time.sleep(0.3)
r = step('GETCUR sel2', 0xA1, 0x81, 0x0200, 0x0200, wlen=60)
if r and r[:1] == b'\xaa':
    print('>>> REPLY OK:', r[:20].hex(' '))

# bad-checksum variant
bad = bytearray(seq[0])
bad[6] ^= 0xFF
bad[7] ^= 0xFF
step('SET bad-ck', 0x21, 0x01, 0x0200, 0x0200, data=bytes(bad))
time.sleep(0.2)
step('GETCUR bad-ck', 0xA1, 0x81, 0x0200, 0x0200, wlen=60)

# EP 0x86 interrupt peek
ep_in = usb.util.find_descriptor(cfg[0], custom_match=lambda e: e.bEndpointAddress == 0x86)
if ep_in is None:
    for intf in cfg:
        ep_in = usb.util.find_descriptor(intf, custom_match=lambda e: e.bEndpointAddress == 0x86)
        if ep_in:
            break
if ep_in:
    try:
        usb.claim_any_interface(dev, 0)
    except Exception:
        pass
    for _ in range(5):
        try:
            d = ep_in.read(64, timeout=1200)
            print('EP6:', bytes(d).hex(' '))
        except usb.core.USBError as ex:
            print('EP6:', ex.errno if ex.errno else ex)
            break
