#!/bin/bash
# OBSBOT diagnostic inside WSL: usbhid load + interface binding state
echo "=== modprobe usbhid ==="
modprobe usbhid 2>&1; echo "modprobe exit: $?"
echo "=== modules now ==="
lsmod | grep -E "usbhid|hid" || echo "(no hid modules)"

echo "=== usb devices (sysfs) ==="
find /sys/bus/usb/devices -maxdepth 2 -name idVendor | while read f; do
  d=$(dirname "$f")
  echo "$(basename $d) $(cat $d/idVendor):$(cat $d/idProduct)"
done

echo "=== OBSBOT interfaces ==="
for i in $(find /sys/bus/usb/devices -maxdepth 1 -name '*3564*' -o -maxdepth 1 -name 'usb*' | grep -v '^usb'); do true; done
dev=$(find /sys/bus/usb/devices -maxdepth 2 -name idVendor -exec grep -l 3564 {} \; | head -1 | xargs -r dirname)
echo "obsbot device dir: $dev"
if [ -n "$dev" ]; then
  echo "config=$([ -e $dev/bConfigurationValue ] && cat $dev/bConfigurationValue) of $(cat $dev/bNumConfigurations 2>/dev/null)"
  for iface in $dev/*:*/; do
    [ -d "$iface" ] || continue
    echo "iface $(basename $iface): class=$(cat $iface/bInterfaceClass 2>/dev/null) sub=$(cat $iface/bInterfaceSubClass 2>/dev/null) proto=$(cat $iface/bInterfaceProtocol 2>/dev/null) driver=$(basename $(readlink -f $iface/driver 2>/dev/null) 2>/dev/null)"
  done
fi

echo "=== hidraw nodes ==="
ls -la /dev/hidraw* 2>/dev/null || echo "none"
ls /sys/class/hidraw 2>/dev/null || true

echo "=== dmesg tail (usb/hid) ==="
dmesg | grep -iE "usb |hid|uvc|3564" | tail -15
