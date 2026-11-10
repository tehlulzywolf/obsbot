#!/bin/bash
cd /mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps
timeout 400 tshark -r cap1.pcap -Y 'usb.device_address==9 && (Response or _ws.col.Info contains "URB_BULK")' -T fields -e frame.number -e _ws.col.Info -e usb.bmRequestType -e usb.bRequest -e usb.wValue -e usb.wIndex -e usb.data_fragment 2>/dev/null > respA.tsv
wc -l respA.tsv
awk -F'\t' '$2 ~ /Response/ {print}' respA.tsv | head -30
echo ===EP6 sample===
awk -F'\t' '$2 ~ /BULK/ && $7 != "" {print "f" $1 ": " substr($7,1,80)}' respA.tsv | head -10
echo ===EP6 unique starts===
awk -F'\t' '$2 ~ /BULK/ && $7 != "" {print substr($7,1,10)}' respA.tsv | sort | uniq -c | sort -rn | head -8
