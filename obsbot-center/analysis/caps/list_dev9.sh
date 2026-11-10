#!/bin/bash
cd /mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps
timeout 300 tshark -r cap1.pcap -Y 'usb.device_address==9' -T fields -e frame.number -e _ws.col.Info 2>/dev/null > dev9_all.txt
wc -l dev9_all.txt
awk -F'\t' '$2 !~ /Isochronous/ && $2 !~ /interrupt/ {print}' dev9_all.txt > dev9_nonoiso.txt
wc -l dev9_nonoiso.txt
awk -F'\t' '{print $2}' dev9_nonoiso.txt | sed 's/^[ \t]*//' | sort | uniq -c | sort -rn | head -16
