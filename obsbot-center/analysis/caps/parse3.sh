#!/bin/bash
cd /mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps
timeout 400 tshark -r cap1.pcap -Y 'usb.device_address==9 && usb.bmRequestType' -T fields \
  -e frame.number -e frame.time_relative -e _ws.col.Info -e usb.bmRequestType -e usb.bRequest -e usb.wValue -e usb.wIndex -e usb.wLength -e usb.data_fragment 2>/dev/null > ctrl9.tsv
echo "LINES: $(wc -l < ctrl9.tsv)"
echo "=== request kinds (wValue.wIndex selector) ==="
awk -F'\t' '{split($3,a," "); print $3, "|", $6, $7}' ctrl9.tsv | sed 's/ Request.*|/|/' | sort | uniq -c | sort -rn | head -15
echo "=== SET CUR payloads (first 8) ==="
awk -F'\t' '$3 ~ /SET CUR/ && $9 != "" && $9 != "-" {printf "f%s t%s wv=%s wi=%s wl=%s\n  %s\n", $1,$2,$6,$7,$8,substr($9,1,140)}' ctrl9.tsv | head -20
echo "=== GET CUR responses (first 8) ==="
awk -F'\t' '$3 ~ /GET CUR/ && $9 != "" && $9 != "-" {printf "f%s t%s wv=%s wi=%s\n  %s\n", $1,$2,$6,$7,substr($9,1,140)}' ctrl9.tsv | head -20
