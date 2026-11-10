#!/bin/bash
cd /mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps
timeout 500 tshark -r cap1.pcap -Y 'usb.device_address==9' -T fields \
  -e frame.number -e frame.time_relative -e usb.urb_type -e usb.transfer_type \
  -e usb.endpoint_address -e usb.bRequestType -e usb.bRequest -e usb.wValue \
  -e usb.wIndex -e usb.wLength -e usb.data > dev9.tsv 2> dev9.err
echo "LINES: $(wc -l < dev9.tsv)"
echo "--- stderr ---"; head -3 dev9.err
echo "--- transfer types ---"; awk -F'\t' '{print $4}' dev9.tsv | sort | uniq -c | sort -rn
echo "--- control requests (bRequestType bRequest wValue wIndex wLength) ---"
awk -F'\t' 'length($7)>0 {print $6, $7, $8, $9, $10}' dev9.tsv | sort | uniq -c | sort -rn | head -25
