#!/usr/bin/env bash
# install.sh - Build and install the epson-p900-escpr-cups Rust driver into CUPS.
#
# Usage:
#   ./install.sh                  # Build release binaries
#   sudo ./install.sh [IP]        # Install into CUPS and optionally register queue
#   sudo ./install.sh --uninstall # Remove driver binaries, PPD generator, polkit policy & Epson ICCs
#                                 # (keeps /etc/epson-p900-escpr-cups/ and custom ICC profiles)
#   sudo ./install.sh --purge     # Uninstall and also remove /etc/epson-p900-escpr-cups/, custom ICCs & queue
#
# SPDX-License-Identifier: GPL-3.0-or-later

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ARG1="${1:-}"
BIN_PATH="$SCRIPT_DIR/target/release/rastertop900escpr"
HELPER_PATH="$SCRIPT_DIR/target/release/epson-p900-media-install"
HELPER_DEST=/usr/libexec/epson-p900-escpr-cups/epson-p900-media-install
POLICY_DEST=/usr/share/polkit-1/actions/de.colorcube.epson-p900-escpr.install-media.policy
SYSTEM_MEDIA_DIR=/etc/epson-p900-escpr-cups
SYSTEM_MEDIA_JSON=/etc/epson-p900-escpr-cups/custom_media.json
OLD_MEDIA_JSON=/usr/share/epson-p900-escpr-cups/custom_media.json
CUSTOM_ICC_DIR=/usr/local/share/color/icc/epson-p900-custom

if [[ "$ARG1" == "-h" || "$ARG1" == "--help" ]]; then
    sed -n '2,9p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
    exit 0
fi

if [[ "$ARG1" == "--uninstall" || "$ARG1" == "--purge" ]]; then
    if [[ $EUID -ne 0 ]]; then
        echo "ERROR: $ARG1 requires root privileges (run: sudo ./install.sh $ARG1)" >&2
        exit 1
    fi
    echo "=== Uninstalling epson-p900-escpr-cups driver files ==="
    rm -f /usr/lib/cups/filter/rastertop900escpr
    rm -f /usr/lib/cups/driver/epson-p900-escpr
    rm -f "$HELPER_DEST"
    rmdir "$(dirname "$HELPER_DEST")" 2>/dev/null || true
    rm -f "$POLICY_DEST"
    rm -rf /usr/share/color/icc/epson-scp900
    rm -rf /usr/share/cups/model/epson-p900-escpr
    rm -f /var/cache/cups/ppds.dat 2>/dev/null || true
    pkill -f scp-dbus-service.py 2>/dev/null || true

    if [[ "$ARG1" == "--purge" ]]; then
        echo "=== Purging custom media configuration, custom ICC profiles, and default queue ==="
        rm -rf "$SYSTEM_MEDIA_DIR"
        rm -rf "$CUSTOM_ICC_DIR"
        lpadmin -x Epson_SC_P900_ESCPR 2>/dev/null || true
        echo "Uninstalled driver and purged custom media configuration."
    else
        echo "Driver binaries, PPD generator, polkit policy, and Epson ICC profiles removed."
        echo "Preserved:"
        echo "  - Custom media configuration: $SYSTEM_MEDIA_DIR/"
        echo "  - Custom ICC profiles:        $CUSTOM_ICC_DIR/"
        echo "  - Existing CUPS queues (remove manually with: sudo lpadmin -x Epson_SC_P900_ESCPR)"
        echo "To remove everything including custom media and profiles, run: sudo ./install.sh --purge"
    fi
    exit 0
fi

PRINTER_IP="$ARG1"
if [[ "$PRINTER_IP" == "--driver-only" || "$PRINTER_IP" == -* ]]; then
    PRINTER_IP=""
fi

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

if [[ $EUID -ne 0 ]]; then
    echo ""
    echo "Build complete."
    echo "  - Install driver only (manual setup in KDE/GNOME/CUPS UI): sudo ./install.sh"
    echo "  - Install driver AND create CUPS queue automatically:       sudo ./install.sh <PRINTER_IP>"
    exit 0
fi

echo "=== 3. Installing filter, PPD generator, media helper, and ICC profiles ==="
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

# PPDs come from the driver program above (generated when a queue is created). Remove the static
# PPDs of older versions so printer setup tools do not list every model twice.
rm -rf /usr/share/cups/model/epson-p900-escpr

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
        /usr/share/color/icc/epson-scp900 >/dev/null 2>&1 || true
fi

# Invalidate CUPS PPD cache and D-Bus driver matcher cache so GUIs see updated PPDs immediately
rm -f /var/cache/cups/ppds.dat 2>/dev/null || true
pkill -f scp-dbus-service.py 2>/dev/null || true

if [[ -n "$PRINTER_IP" ]]; then
    echo "=== 4. Registering CUPS queue 'Epson_SC_P900_ESCPR' at socket://${PRINTER_IP}:9100 ==="
    lpadmin -p Epson_SC_P900_ESCPR -E \
        -v "socket://${PRINTER_IP}:9100" \
        -m epson-p900-escpr:SC-P900 \
        -D "Epson SureColor SC-P900 (Official ICC + ESC/P-R Lv5)"
    echo "Registered CUPS printer 'Epson_SC_P900_ESCPR'."
else
    echo "Driver files installed into CUPS (no printer queue created)."
    echo "Note: Close and reopen KDE System Settings (systemsettings) so its cached PPD list reloads."
    echo "You can now add the printer in KDE System Settings / GNOME / CUPS (http://localhost:631)"
    echo "and select 'EPSON SC-P900 Series (SureColor) - Official ICC + ESC/P-R Lv5 (high quality)'."
fi

# Refresh the PPDs of existing queues (new driver version, installed custom media)
"$HELPER_DEST" --refresh-queues || echo "WARNING: could not refresh queue PPDs"

echo "Installation finished successfully."
