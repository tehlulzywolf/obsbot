#!/bin/bash
cd /mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps
timeout 400 tshark -r cap1.pcap -Y 'usb.device_address==9 && (usb.bmRequestType || usb.data_fragment)' -T fields \
  -e frame.number -e frame.time_relative -e _ws.col.Info -e usb.bmRequestType -e usb.bRequest -e usb.wValue -e usb.wIndex -e usb.wLength -e usb.data_fragment 2>/dev/null > ctrl_raw.tsv
echo "LINES: $(wc -l < ctrl_raw.tsv)"
echo "=== SET CUR wValue/wIndex/wLength values seen ==="
awk -F'\t' '$3 ~ /SET CUR/ {print "wv="$6" wi="$7" wl="$8}' ctrl_raw.tsv | sort | uniq -c
echo "=== GET CUR values seen ==="
awk -F'\t' '$3 ~ /GET CUR/ {print "wv="$6" wi="$7" wl="$8}' ctrl_raw.tsv | sort | uniq -c
echo
echo "=== SET CUR payloads (first 30 unique-ish) ==="
awk -F'\t' '$3 ~ /SET CUR/ {print "SET f" $1 " t=" $2 " wv=" $6}' ctrl_raw.tsv | head -3
# find data-stage frames right after SET CURs
awk -F'\t' 'BEGIN{FS=OFS="\t"} {print $1, $2, $3, $9}' ctrl_raw.tsv | grep -vE "^[0-9]+\s+[0-9.]+\s+\s*$" | head -0
echo "=== payload-bearing frames (frame, info, data) ==="
awk -F'\t' '$9 != "" && $9 != "-" {print "f" $1 " [" $3 "] " substr($9,1,132)}' ctrl_raw.tsv | head -40
