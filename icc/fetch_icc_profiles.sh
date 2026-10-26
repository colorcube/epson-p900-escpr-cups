#!/usr/bin/env bash
# ==============================================================================
# fetch_icc_profiles.sh
# Downloads or extracts the 15 official Epson SC-P900 / SC-P700 ICC profiles
# from the official Epson Windows (.exe) or macOS (.dmg) driver package.
#
# Note: The 15 ICC profiles in the Windows package (EE2NE_00.ICC..EE2NE_14.ICC)
# and macOS package (Epson_SC-P900_700_*.icc) are 100% byte-for-byte identical.
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")" && pwd)"
TMP_DIR="$SCRIPT_DIR/_tmp_extract"

# Official Epson Windows x64 driver URL (~41 MB, requires only 7z to extract)
WIN_DRIVER_URL="https://download.ebz.epson.net/dsc/du/02/DriverDownloadInfo.do?LG2=EN&CN2=US&CTI=4&PRN=SC-P900%20Series&OSC=W64"
FALLBACK_WIN_DIRECT_URL="https://ftp.epson.com/drivers/SCP900_STD_WW_612200_W64.exe"

declare -A ICC_MAP=(
    ["EE2NE_00.ICC"]="Epson_SC-P900_700_ArchivalMattePaper.icc"
    ["EE2NE_01.ICC"]="Epson_SC-P900_700_Baryta.icc"
    ["EE2NE_02.ICC"]="Epson_SC-P900_700_CanvasMatte.icc"
    ["EE2NE_03.ICC"]="Epson_SC-P900_700_CanvasSatin.icc"
    ["EE2NE_04.ICC"]="Epson_SC-P900_700_EpsonPremiumGlossy.icc"
    ["EE2NE_05.ICC"]="Epson_SC-P900_700_EpsonPremiumSemigloss.icc"
    ["EE2NE_06.ICC"]="Epson_SC-P900_700_FineArtMatte.icc"
    ["EE2NE_07.ICC"]="Epson_SC-P900_700_PhotoPaperGlossy.icc"
    ["EE2NE_08.ICC"]="Epson_SC-P900_700_PremiumLusterPhotoPaper.icc"
    ["EE2NE_09.ICC"]="Epson_SC-P900_700_Standard.icc"
    ["EE2NE_10.ICC"]="Epson_SC-P900_700_UltraSmoothFineArtPaper.icc"
    ["EE2NE_11.ICC"]="Epson_SC-P900_700_VelvetFineArtPaper.icc"
    ["EE2NE_12.ICC"]="Epson_SC-P900_700_Washi.icc"
    ["EE2NE_13.ICC"]="Epson_SC-P900_700_WatercolorPaperRadiantWhite.icc"
    ["EE2NE_14.ICC"]="Epson_SC-P900_700_ProofingPaperWhiteSemimatte.icc"
)

cleanup() {
    rm -rf "$TMP_DIR"
}
trap cleanup EXIT

command -v 7z >/dev/null 2>&1 || {
    echo "Error: '7z' (p7zip) is required to extract the driver archive." >&2
    exit 1
}

ARCHIVE="${1:-}"

# Auto-detect local driver archive in workspace if not explicitly provided
if [[ -z "$ARCHIVE" ]]; then
    LOCAL_WIN="$SCRIPT_DIR/../../Epson-SC-P900-Treiber/win_driver/SCP900_STD_WW_612200_W64.exe"
    if [[ -f "$LOCAL_WIN" ]]; then
        ARCHIVE="$LOCAL_WIN"
        echo "Using local Windows driver archive: $ARCHIVE"
    fi
fi

mkdir -p "$TMP_DIR"

if [[ -z "$ARCHIVE" ]]; then
    ARCHIVE="$TMP_DIR/SCP900_STD_WW_612200_W64.exe"
    echo "Downloading official Epson SC-P900 Windows driver (~41 MB)..."
    curl -fL "$FALLBACK_WIN_DIRECT_URL" -o "$ARCHIVE"
fi

if [[ ! -f "$ARCHIVE" ]]; then
    echo "Error: Archive not found: $ARCHIVE" >&2
    exit 1
fi

echo "Extracting ICC profiles from $ARCHIVE ..."
7z e -y "$ARCHIVE" -r "EE2NE_*.IC_" -o"$TMP_DIR" >/dev/null

count=0
for ic_cab in "$TMP_DIR"/EE2NE_*.IC_; do
    [[ -f "$ic_cab" ]] || continue
    7z e -y "$ic_cab" -o"$TMP_DIR" >/dev/null
done

for win_name in "${!ICC_MAP[@]}"; do
    canon_name="${ICC_MAP[$win_name]}"
    src_file="$TMP_DIR/$win_name"
    if [[ -f "$src_file" ]]; then
        cp -f "$src_file" "$SCRIPT_DIR/$canon_name"
        ln -sf "$canon_name" "$SCRIPT_DIR/$win_name"
        count=$((count + 1))
    else
        echo "Warning: $win_name was not found in archive" >&2
    fi
done

echo "Successfully extracted $count / 15 official Epson ICC profiles into $SCRIPT_DIR"
