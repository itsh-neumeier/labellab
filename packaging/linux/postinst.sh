#!/bin/sh
# Apply the udev rule for USB printer access without a reboot.
if command -v udevadm >/dev/null 2>&1; then
  udevadm control --reload-rules || true
  udevadm trigger --subsystem-match=usb || true
fi
exit 0
