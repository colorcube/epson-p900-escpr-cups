#!/bin/sh
# Post-install hook for epson-p900-escpr-cups (.deb / .rpm)
# SPDX-License-Identifier: GPL-3.0-or-later
set -e

OLD_MEDIA_JSON=/usr/share/epson-p900-escpr-cups/custom_media.json
SYSTEM_MEDIA_JSON=/etc/epson-p900-escpr-cups/custom_media.json

mkdir -p /etc/epson-p900-escpr-cups
mkdir -p /usr/share/color/icc/epson-scp900

if [ -f "$OLD_MEDIA_JSON" ] && [ ! -f "$SYSTEM_MEDIA_JSON" ]; then
    cp -p "$OLD_MEDIA_JSON" "$SYSTEM_MEDIA_JSON" || true
fi
rm -f "$OLD_MEDIA_JSON"
rmdir /usr/share/epson-p900-escpr-cups 2>/dev/null || true

# Remove static PPDs from older versions so printer setup GUIs do not list models twice
rm -rf /usr/share/cups/model/epson-p900-escpr

if command -v restorecon >/dev/null 2>&1; then
    restorecon -RF \
        /usr/lib/cups/filter/rastertop900escpr \
        /usr/lib/cups/driver/epson-p900-escpr \
        /usr/libexec/epson-p900-escpr-cups \
        /usr/bin/epson-p900-fetch-icc-profiles \
        /etc/epson-p900-escpr-cups \
        /usr/share/color/icc/epson-scp900 >/dev/null 2>&1 || true
fi

rm -f /var/cache/cups/ppds.dat 2>/dev/null || true
pkill -f scp-dbus-service.py 2>/dev/null || true

if [ -x /usr/libexec/epson-p900-escpr-cups/epson-p900-media-install ]; then
    /usr/libexec/epson-p900-escpr-cups/epson-p900-media-install --refresh-queues >/dev/null 2>&1 || true
fi

if [ ! -f /usr/share/color/icc/epson-scp900/Epson_SC-P900_700_Standard.icc ]; then
    echo "========================================================================"
    echo "epson-p900-escpr-cups installed."
    echo "Official Epson ICC profiles are not bundled in this package (copyright)."
    echo "To download and install the 15 Epson SC-P900 / SC-P700 ICC profiles into"
    echo "/usr/share/color/icc/epson-scp900/, run:"
    echo ""
    echo "  sudo epson-p900-fetch-icc-profiles"
    echo "========================================================================"
fi
