#!/bin/bash
cd /mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps
timeout 300 tshark -r cap1.pcap -Y 'usb.device_address==9 && usbdst_host' -x 2>/dev/null > ctrl_ep0_x.txt
timeout 300 tshark -r cap1.pcap -Y 'usb.device_address==9' -T fields -e frame.number -e _ws.col.Info 2>/dev/null > dev9_all.txt
wc -l ctrl_ep0_x.txt dev9_all.txt
grep -c "Response" dev9_all.txt
