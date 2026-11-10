#!/usr/bin/env bash
# OBSBOT Tiny 2 Lite — live protocol test + sniff capture for Linux.
# Run as: sudo bash run_test.sh   (needs sudo for usbmon + hidraw unless udev rule installed)
set -u
cd "$(dirname "$0")"

# 1) sanity: device present?
if ! lsusb -d 3564:fef9 >/dev/null 2>&1; then
  echo "!! OBSBOT (3564:fef9) not on USB bus. Plug it in first."; exit 1
fi
lsusb -d 3564:fef9
echo "hidraw nodes:"; for h in /sys/class/hidraw/*; do
  d=$(basename "$(readlink -f "$h/device" 2>/dev/null)" 2>/dev/null)
  v=$(cat "$h/device/../idVendor" 2>/dev/null || cat "$h/../idVendor" 2>/dev/null)
  p=$(cat "$h/device/../idProduct" 2>/dev/null || cat "$h/../idProduct" 2>/dev/null)
  [ "$v" = "3564" ] && echo "  /dev/$(basename $h)  vid:pid=$v:$p"
done

# 2) kernel-side wire log in parallel
bus=$(lsusb -d 3564:fef9 | awk '{print $2}' | tr -d '@')
dev=$(lsusb -d 3564:fef9 | awk '{print $4}' | tr -d '@')
mon=$(printf "%x" $bus)u
monfile=usbmon_${bus}_${dev}.txt
if [ -r /sys/kernel/debug/usb/usbmon/$mon ]; then
  ( cat /sys/kernel/debug/usb/usbmon/$mon > "$monfile" & MONPID=$!
    sleep 1; echo "usbmon capturing -> $monfile" )
else
  ( mount -t debugfs none /sys/kernel/debug 2>/dev/null
    cat /sys/kernel/debug/usb/usbmon/$mon > "$monfile" 2>/dev/null & MONPID=$! )
fi

# 3) run the live demo (gimbal attitude, presets, stop, boot status)
echo; echo "== hello_gimbal =="
chmod +x ./hello_gimbal
./hello_gimbal
rc=$?

sleep 1; kill $MONPID 2>/dev/null; wait 2>/dev/null
echo; echo "sniff saved: $(pwd)/$monfile ($(wc -l < "$monfile" 2>/dev/null || echo 0) lines)"
echo "Bring back $monfile + the hello_gimbal output above."
exit $rc
