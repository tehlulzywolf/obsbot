#!/bin/bash
# Dump every dev9 control transfer (setup+data) as hex, save for python parsing
cd /mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps
timeout 400 tshark -r cap1.pcap -Y 'usb.device_address==9 && usb.bmRequestType' -x 2>/dev/null > ctrl9_x.txt
wc -c ctrl9_x.txt
