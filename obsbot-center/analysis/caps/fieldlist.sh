#!/bin/bash
tshark -G fields 2>/dev/null | awk -F'\t' '{print $2}' | grep -E '^usb\.' | grep -Ei 'request|value|index|length|data|payload|fragment' | sort -u > /tmp/usbfields.txt
wc -l /tmp/usbfields.txt
cat /tmp/usbfields.txt
