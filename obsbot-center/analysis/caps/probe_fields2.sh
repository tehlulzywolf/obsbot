#!/bin/bash
cd /mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps
for f in usb.bmRequestType usb.bRequestType usb.bRequest usb.wValue usb.wIndex usb.wLength usb.frame_length; do
  msg=$(timeout 90 tshark -r cap1.pcap -c 50 -Y "usb.device_address==9" -T fields -e "$f" 2>&1 >/dev/null | tail -1)
  echo "$f => ${msg:-OK}"
done
echo "=== try combined ==="
timeout 300 tshark -r cap1.pcap -Y 'usb.device_address==9 && usb.bmRequestType' -T fields -e frame.number -e usb.bmRequestType -e usb.bRequest -e usb.wValue -e usb.wIndex -e usb.wLength 2>&1 | head -4
