#!/bin/bash
cd /mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps
echo "=== urb_type values ==="
timeout 300 tshark -r cap1.pcap -Y 'usb.device_address==9' -T fields -e usb.urb_type 2>/dev/null | sort | uniq -c
echo "=== with data? (urb_type x has-data) ==="
timeout 300 tshark -r cap1.pcap -Y 'usb.device_address==9' -T fields -e usb.urb_type -e usb.data_fragment 2>/dev/null | awk -F'\t' '{print $1, ($2==""?"nodata":"DATA")}' | sort | uniq -c
