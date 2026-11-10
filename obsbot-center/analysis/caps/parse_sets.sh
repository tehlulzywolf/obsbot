#!/bin/bash
cd /mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps
# All SETUP + data stages for dev9 control traffic, with payload hex
timeout 400 tshark -r cap1.pcap -Y 'usb.device_address==9 && (usb.bmRequestType || (usb.urb_type && usb.data_fragment))' -T fields \
  -e frame.number -e frame.time_relative -e _ws.col.Info -e usb.bmRequestType -e usb.bRequest -e usb.wValue -e usb.wIndex -e usb.wLength -e usb.data_fragment 2>/dev/null > ctrl_raw.tsv
wc -l ctrl_raw.tsv
echo "=== SET CUR payloads (selector in wValue) ==="
awk -F'\t' '$3 ~ /SET CUR/ {print $1, $2, "wv="$6, "wi="$7, "wl="$8; if ($9!="") print "   data:", substr($9,1,140)}' ctrl_raw.tsv | head -60
