#!/usr/bin/env bash
# install.sh - Build and install the epson-p900-escpr-cups driver into CUPS.
#
# Usage:
#   sudo ./install.sh [PRINTER_IP]
#
# SPDX-License-Identifier: GPL-3.0-or-later

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PRINTER_IP="${1:-}"

echo "=== 1. Compiling RHV2 C accelerator (librhv2.so) ==="
if command -v gcc >/dev/null 2>&1; then
    gcc -O3 -shared -fPIC "$SCRIPT_DIR/src/rhv2.c" -o "$SCRIPT_DIR/src/librhv2.so"
    echo "Compiled $SCRIPT_DIR/src/librhv2.so"
else
    echo "WARN: gcc not found; filter will use pure-Python RHV2 fallback."
fi

echo "=== 2. Checking Epson ICC profiles ==="
if ! ls "$SCRIPT_DIR/icc/"*.icc >/dev/null 2>&1; then
    echo "Extracting Epson ICC profiles via icc/fetch_icc_profiles.sh ..."
    bash "$SCRIPT_DIR/icc/fetch_icc_profiles.sh"
fi

echo "=== 3. Generating CUPS PPD files ==="
python3 "$SCRIPT_DIR/scripts/generate_ppd.py"

if [[ $EUID -ne 0 ]]; then
    echo ""
    echo "Build complete. Run 'sudo ./install.sh [PRINTER_IP]' to install into /usr/lib/cups/filter and register the CUPS queue."
    exit 0
fi

echo "=== 4. Installing filter, modules, PPDs, and ICC profiles ==="
install -d /usr/share/epson-p900-escpr-cups/src
install -m 644 "$SCRIPT_DIR/src/escpr_protocol.py"  /usr/share/epson-p900-escpr-cups/src/
install -m 644 "$SCRIPT_DIR/src/icc_manager.py"     /usr/share/epson-p900-escpr-cups/src/
install -m 644 "$SCRIPT_DIR/src/media_installer.py" /usr/share/epson-p900-escpr-cups/src/
install -m 644 "$SCRIPT_DIR/src/rhv2.c"             /usr/share/epson-p900-escpr-cups/src/
if [[ -f "$SCRIPT_DIR/src/librhv2.so" ]]; then
    install -m 755 "$SCRIPT_DIR/src/librhv2.so"     /usr/share/epson-p900-escpr-cups/src/
fi
if [[ -f "$SCRIPT_DIR/custom_media.json" ]]; then
    install -m 644 "$SCRIPT_DIR/custom_media.json"  /usr/share/epson-p900-escpr-cups/custom_media.json
fi

install -d /usr/lib/cups/filter
install -m 755 "$SCRIPT_DIR/filter/rastertop900escpr" /usr/lib/cups/filter/rastertop900escpr

install -d /usr/share/cups/model/epson-p900-escpr
install -m 644 "$SCRIPT_DIR/ppd/Epson-SC-P900-ESCPR.ppd" /usr/share/cups/model/epson-p900-escpr/
install -m 644 "$SCRIPT_DIR/ppd/Epson-SC-P700-ESCPR.ppd" /usr/share/cups/model/epson-p900-escpr/

install -d /usr/share/color/icc/epson-scp900
shopt -s nullglob
cp -df "$SCRIPT_DIR/icc/"*.icc "$SCRIPT_DIR/icc/"*.ICC /usr/share/color/icc/epson-scp900/
chmod 644 /usr/share/color/icc/epson-scp900/*.icc 2>/dev/null || true

if command -v restorecon >/dev/null 2>&1; then
    restorecon -RF \
        /usr/lib/cups/filter/rastertop900escpr \
        /usr/share/epson-p900-escpr-cups \
        /usr/share/cups/model/epson-p900-escpr \
        /usr/share/color/icc/epson-scp900 >/dev/null 2>&1 || true
fi

if [[ -n "$PRINTER_IP" ]]; then
    echo "=== 5. Registering CUPS queue 'Epson_SC_P900_ESCPR' at socket://${PRINTER_IP}:9100 ==="
    lpadmin -p Epson_SC_P900_ESCPR -E \
        -v "socket://${PRINTER_IP}:9100" \
        -P /usr/share/cups/model/epson-p900-escpr/Epson-SC-P900-ESCPR.ppd \
        -D "Epson SureColor SC-P900 (Official ICC + ESC/P-R Lv5)"
    echo "Registered CUPS printer 'Epson_SC_P900_ESCPR'."
fi

echo "Installation finished successfully."
