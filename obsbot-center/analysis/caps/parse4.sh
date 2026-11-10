#!/bin/bash
cd /mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps
timeout 400 tshark -r cap1.pcap -Y 'usb.device_address==9 && usb.bmRequestType' -T fields \
  -e frame.number -e frame.time_relative -e _ws.col.Info -e usb.bmRequestType -e usb.bRequest -e usb.data_fragment 2>/dev/null > ctrlA.tsv
# wValue/wIndex invalid in this dissector build; Info column shows [Unit x control y] which is enough
echo "LINES: $(wc -l < ctrlA.tsv)"
echo "=== kinds x count ==="
awk -F'\t' '{print $3}' ctrlA.tsv | sed 's/  */ /g' | sort | uniq -c | sort -rn | head
echo "=== sample SET CUR with payloads ==="
awk -F'\t' '$3 ~ /SET CUR/ {print "f" $1 " t" $2 " [" $3 "] bmt=" $4 " breq=" $5; print "   DATA: " substr($6,1,150)}' ctrlA.tsv | head -24
echo "=== sample GET CUR responses ==="
awk -F'\t' '$3 ~ /GET CUR/ {print "f" $1 " t" $2 " [" $3 "] bmt=" $4; print "   DATA: " substr($6,1,150)}' ctrlA.tsv | head -24
cp ctrlA.tsv /tmp/ctrlA.tsv 2>/dev/null || true
