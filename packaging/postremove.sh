#!/bin/sh
# Post-remove hook for epson-p900-escpr-cups (.deb / .rpm)
# SPDX-License-Identifier: GPL-3.0-or-later
set -e

rm -f /var/cache/cups/ppds.dat 2>/dev/null || true
pkill -f scp-dbus-service.py 2>/dev/null || true
