#!/usr/bin/env bash
# install.sh - Build and install the epson-p900-escpr-cups Rust driver into CUPS.
#
# Usage:
#   ./install.sh              # Build release binary and generate PPDs
#   sudo ./install.sh [IP]    # Install into CUPS and optionally register queue
#
# SPDX-License-Identifier: GPL-3.0-or-later

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PRINTER_IP="${1:-}"
if [[ "$PRINTER_IP" == "--driver-only" || "$PRINTER_IP" == -* ]]; then
    PRINTER_IP=""
fi
BIN_PATH="$SCRIPT_DIR/target/release/rastertop900escpr"
HELPER_PATH="$SCRIPT_DIR/target/release/epson-p900-media-install"
HELPER_DEST=/usr/libexec/epson-p900-escpr-cups/epson-p900-media-install
SYSTEM_MEDIA_JSON=/etc/epson-p900-escpr-cups/custom_media.json
OLD_MEDIA_JSON=/usr/share/epson-p900-escpr-cups/custom_media.json

echo "=== 1. Building Rust CUPS filter & CLI (rastertop900escpr) ==="
if [[ $EUID -eq 0 && -n "${SUDO_USER:-}" ]]; then
    USER_HOME="$(getent passwd "$SUDO_USER" | cut -d: -f6)"
    sudo -u "$SUDO_USER" env HOME="$USER_HOME" PATH="$USER_HOME/.cargo/bin:$PATH" \
        bash -c "cd '$SCRIPT_DIR' && (cargo build --release --offline || cargo build --release)"
else
    export PATH="$HOME/.cargo/bin:$PATH"
    (cd "$SCRIPT_DIR" && (cargo build --release --offline || cargo build --release))
fi

echo "=== 2. Checking Epson ICC profiles ==="
if ! ls "$SCRIPT_DIR/icc/"*.icc >/dev/null 2>&1; then
    echo "Extracting Epson ICC profiles via icc/fetch_icc_profiles.sh ..."
    if [[ $EUID -eq 0 && -n "${SUDO_USER:-}" ]]; then
        sudo -u "$SUDO_USER" bash "$SCRIPT_DIR/icc/fetch_icc_profiles.sh"
    else
        bash "$SCRIPT_DIR/icc/fetch_icc_profiles.sh"
    fi
fi

echo "=== 3. Generating CUPS PPD files ==="
if [[ $EUID -eq 0 && -n "${SUDO_USER:-}" ]]; then
    sudo -u "$SUDO_USER" "$BIN_PATH" --generate-ppd --ppd-dir "$SCRIPT_DIR/ppd"
else
    "$BIN_PATH" --generate-ppd --ppd-dir "$SCRIPT_DIR/ppd"
fi

if [[ $EUID -ne 0 ]]; then
    echo ""
    echo "Build complete."
    echo "  - Install driver only (manual setup in KDE/GNOME/CUPS UI): sudo ./install.sh"
    echo "  - Install driver AND create CUPS queue automatically:       sudo ./install.sh <PRINTER_IP>"
    exit 0
fi

echo "=== 4. Installing filter, PPD generator, media helper, PPDs, and ICC profiles ==="
rm -rf /usr/share/epson-p900-escpr-cups/src

install -d /usr/lib/cups/filter
install -m 755 "$BIN_PATH" /usr/lib/cups/filter/rastertop900escpr

# CUPS driver program: serves PPDs incl. the system's custom media to lpinfo/lpadmin -m
install -d /usr/lib/cups/driver
ln -sf /usr/lib/cups/filter/rastertop900escpr /usr/lib/cups/driver/epson-p900-escpr

# Privileged helper for ep-media-installer "Für das System installieren" (pkexec + polkit)
install -d "$(dirname "$HELPER_DEST")"
install -m 755 "$HELPER_PATH" "$HELPER_DEST"
install -d /usr/share/polkit-1/actions
install -m 644 "$SCRIPT_DIR/share/polkit-1/actions/de.colorcube.epson-p900-escpr.install-media.policy" \
    /usr/share/polkit-1/actions/

# System custom media database (written by the helper). Older versions installed the media list
# to /usr/share; move it so existing custom media keep working.
install -d -m 755 /etc/epson-p900-escpr-cups
if [[ -f "$OLD_MEDIA_JSON" && ! -f "$SYSTEM_MEDIA_JSON" ]]; then
    install -m 644 "$OLD_MEDIA_JSON" "$SYSTEM_MEDIA_JSON"
    echo "Migrated $OLD_MEDIA_JSON -> $SYSTEM_MEDIA_JSON"
fi
rm -f "$OLD_MEDIA_JSON"
rmdir /usr/share/epson-p900-escpr-cups 2>/dev/null || true

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
        /usr/lib/cups/driver/epson-p900-escpr \
        "$(dirname "$HELPER_DEST")" \
        /etc/epson-p900-escpr-cups \
        /usr/share/cups/model/epson-p900-escpr \
        /usr/share/color/icc/epson-scp900 >/dev/null 2>&1 || true
fi

# Invalidate CUPS PPD cache and D-Bus driver matcher cache so GUIs see updated PPDs immediately
rm -f /var/cache/cups/ppds.dat 2>/dev/null || true
pkill -f scp-dbus-service.py 2>/dev/null || true

if [[ -n "$PRINTER_IP" ]]; then
    echo "=== 5. Registering CUPS queue 'Epson_SC_P900_ESCPR' at socket://${PRINTER_IP}:9100 ==="
    lpadmin -p Epson_SC_P900_ESCPR -E \
        -v "socket://${PRINTER_IP}:9100" \
        -m epson-p900-escpr:SC-P900 \
        -D "Epson SureColor SC-P900 (Official ICC + ESC/P-R Lv5)" \
    || lpadmin -p Epson_SC_P900_ESCPR -E \
        -v "socket://${PRINTER_IP}:9100" \
        -P /usr/share/cups/model/epson-p900-escpr/Epson-SC-P900-ESCPR.ppd \
        -D "Epson SureColor SC-P900 (Official ICC + ESC/P-R Lv5)"
    echo "Registered CUPS printer 'Epson_SC_P900_ESCPR'."
else
    echo "Driver files installed into CUPS (no printer queue created)."
    echo "Note: Close and reopen KDE System Settings (systemsettings) so its cached PPD list reloads."
    echo "You can now add the printer in KDE System Settings / GNOME / CUPS (http://localhost:631)"
    echo "and select 'EPSON SC-P900 Series (SureColor) - Official ICC + ESC/P-R Lv5'."
fi

# Refresh the PPDs of existing queues (new driver version, installed custom media)
"$HELPER_DEST" --refresh-queues || echo "WARNING: could not refresh queue PPDs"

echo "Installation finished successfully."
