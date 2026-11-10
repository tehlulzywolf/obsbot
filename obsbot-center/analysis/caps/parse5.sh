#!/bin/bash
cd /mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps
timeout 400 tshark -r cap1.pcap -Y 'usb.device_address==9 && usb.bmRequestType' -T fields \
  -e frame.number -e frame.time_relative -e _ws.col.Info -e usb.bmRequestType -e usb.bRequest -e usb.data_fragment 2>err4.txt > ctrlA.tsv
echo "LINES: $(wc -l < ctrlA.tsv)"
echo "--- stderr ---"; cat err4.txt | head -5
