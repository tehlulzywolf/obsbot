#!/bin/bash
cd /mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps
for f in usb.data usb.Data usb.PayloadData usb.data_fragment usb.payload usb.urb_data; do
  msg=$(timeout 90 tshark -r cap1.pcap -c 50 -T fields -e "$f" 2>&1 >/dev/null | head -2 | tr '\n' ' ')
  echo "$f => ${msg:-OK}"
done
