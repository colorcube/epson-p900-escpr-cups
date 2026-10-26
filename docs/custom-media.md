# Custom Media & Third-Party Papers (`.emx` / NVRAM Slots `1..30`)

The Epson SureColor SC-P900 and SC-P700 support third-party papers (such as
Photolux, Hahnemühle, Canson, Ilford) and custom media configurations registered
into the printer's **30 non-volatile memory slots (`Slot 1..30`)** via SNMPv1 or
imported from `.emx` packages.

This driver supports custom media in two ways:
1. **Mode 2 — Hardware NVRAM Slots (`1..30`) & `.emx` Packages via `scripts/epson_media_installer.py` + `custom_media.json`:**
   Reads, displays, writes, and clears the 30 NVRAM slots directly on the
   SC-P900 (`10.10.10.124`), imports `.emx` files (or downloads them from
   Epson's Cloud Content Server), extracts `.icc` profiles, and writes
   `custom_media.json` so all registered papers appear in the CUPS PPD
   (`*MediaType` + `*cupsICCProfile`) and in **LumaPrint**.
2. **Mode 1 — Dynamic Per-Job Overrides (`lp -o ...` / CLI flags):**
   Allows passing `UserMediaID`, `PrintDuty` (`-50..+50%`), `DryingTime`,
   `BottomEdgePriority`, `RubbingReduction`, and `IgnorePanelMismatch` directly
   per print job without modifying the PPD or occupying an NVRAM slot.

---

## How the SC-P900 handles custom media internally

When printing on a custom paper, the SC-P900 firmware expects **both** a base
media reference and the custom parameters in the ESC/P-R Level 5 header:

1. **Base Media (`ESC q setq` byte 0 = `sn_id`):** Must be set to the underlying
   Epson reference media (`LegacyBaseMedia`, Tag `0x02` in NVRAM; e.g. `13` =
   *Premium Luster*, `25` = *Velvet Fine Art Paper*, `174` = *Baryta*). The
   firmware uses this base ID to select **Photo Black (PK) vs. Matte Black (MK)**
   and the internal halftoning LUT.
2. **Custom NVRAM Slot (`ESC q setl`):** Sends the 4-byte big-endian **32-bit
   `MA_ID`** (`MediaID`, Tag `0x24` stored in the printer's NVRAM slot, e.g.
   `915907607` for Slot 1, or `230009156` for Epson EMX packages) right after
   `ESC u chku`.
   - Disassembly of macOS `ESCPLib` (`0xa2860..0xa2933`) proved that driver UI
     slot IDs `1901..1930` (`0x76d..0x78a`) are translated via the
     `MediaAssistant` table (`MA_ID`) to the 32-bit `MediaID` before emitting
     `ESC q setl`.
   - `escpr_protocol.py` (`resolve_user_media_id()`) automatically resolves slot
     numbers `1..30` and UI IDs `1901..1930` to the 32-bit `user_media_id`
     (`MA_ID`) from `custom_media.json`, or passes a 32-bit `MA_ID` through
     directly.
3. **Extended Mechanical Flags (`ESC m seti`):** Configures `RubbingReduction`
   and `BottomEdgePriority` (print quality priority up to the bottom edge).
4. **Color Density / Print Duty (`ESC p setq` byte 5):** Signed 8-bit integer
   (`-50..+50` %) adjusting total ink limit (`InkDensity - 100` from `.emx`).
5. **Drying Time (`REMOTE1 DR`):** Drying time per printhead pass (`0..255`).
6. **Panel Mismatch Bypass (`ESC u chku` byte 0 = `1`):** Enabled by default so
   the printer starts immediately even if a different media type is currently
   selected on the front panel.

---

## Managing NVRAM Slots & `.emx` Files (`scripts/epson_media_installer.py`)

`scripts/epson_media_installer.py` (backed by `src/media_installer.py`) is a
native Linux replacement for the Windows/macOS **Epson Media Installer
(`v2.3.0`)**.

### 1. List registered slots on the printer (`1..30`)

```bash
python3 scripts/epson_media_installer.py list-slots --host 10.10.10.124
```

Example output from the live SC-P900 (`10.10.10.124`):

```text
Slot  UI ID  32-bit MA_ID  BaseSN  Gap                     Thick  Dry  CutOff  RollOff  Media Name (Printer LCD)
----------------------------------------------------------------------------------------------------------------
   1   1901     915907607      13  0.9 mm (Auto/Narrow)   0.27mm    0      +0       +0  Example Luster 250 [13 (PremiumLuster)]
   2   1902     918927133      25  2.1 mm (Wide)          0.48mm    0      +0       +0  Example Fine Art Rag [25 (VelvetFineArt)]
   3   1903     917232635      25  2.1 mm (Wide)          0.48mm    0      +0       +0  Example Smooth Rag [25 (VelvetFineArt)]
   4   1904     919933176      25  2.1 mm (Wide)          0.48mm    0      +0       +0  Example Matt Fibre [25 (VelvetFineArt)]
   5   1905     917942035     174  1.5/1.6 mm (Standard)  0.30mm    0      +0       +0  Example Baryta Fibre [174 (Baryta)]
```

### 2. Sync printer NVRAM slots into `custom_media.json` and regenerate PPDs

```bash
python3 scripts/epson_media_installer.py sync --host 10.10.10.124 --regenerate-ppd
```

### 3. Inspect or import a local `.emx` package

```bash
# Inspect XML + CPB + ICC metadata inside an .emx file:
python3 scripts/epson_media_installer.py inspect-emx Paper.emx

# Import into custom_media.json, extract ICC to icc/ and ~/.local/share/color/icc/,
# optionally write to printer NVRAM slot 6, and regenerate CUPS PPDs:
python3 scripts/epson_media_installer.py import-emx Paper.emx \
    --slot 6 --host 10.10.10.124 --write-printer --regenerate-ppd
```

### 4. Write or clear a custom media slot on the printer directly

```bash
# Write custom parameters to Slot 10:
python3 scripts/epson_media_installer.py write-slot \
    --host 10.10.10.124 \
    --slot 10 \
    --name "Canson Baryta Photographique II" \
    --base-media Baryta \
    --platen-gap 15 \
    --thickness-mm 0.31 \
    --dry-time 0 \
    --cut-offset 0 \
    --sync --regenerate-ppd

# Clear Slot 10:
python3 scripts/epson_media_installer.py clear-slot \
    --host 10.10.10.124 --slot 10 --sync --regenerate-ppd
```

### 5. Browse and download `.emx` packages from Epson's Cloud Server

```bash
# List downloadable .emx packages for SC-P900 (scp9xx):
python3 scripts/epson_media_installer.py cloud-list --emx-only

# Download an .emx package by EMX ID and import it:
python3 scripts/epson_media_installer.py cloud-download 230009156 --import --regenerate-ppd
```

---

## Printing with Registered Custom Media or Dynamic Overrides

### Via CUPS (`lp`) using a synced slot key

```bash
lp -d Epson_SC_P900_ESCPR \
   -o PageSize=A4 \
   -o InputSlot=FrontFineArt \
   -o MediaType=Slot01_Example_Luster_250 \
   -o PrintQuality=Max \
   photo.tif
```

### Via CUPS (`lp`) using dynamic per-job overrides (Mode 1)

```bash
lp -d Epson_SC_P900_ESCPR \
   -o PageSize=A4 \
   -o InputSlot=FrontFineArt \
   -o MediaType=Baryta \
   -o PrintQuality=Max \
   -o EpsonColorMode=NoColorAdjustment \
   -o UserMediaID=1 \
   -o PrintDuty=-5 \
   -o DryingTime=2 \
   -o BottomEdgePriority=true \
   photo_profiled.tif
```

### Via CLI (`rastertop900escpr`)

```bash
# Using a synced slot key (automatically sets sn_id, 32-bit MA_ID, ICC profile, etc.):
./filter/rastertop900escpr \
    --image photo.tif \
    --media Slot05_Example_Baryta_Fibre \
    --input-slot FrontFineArt \
    --quality Max \
    --send 10.10.10.124:9100

# Or using on-the-fly parameters (--user-media-id accepts 1..30, 1901..1930, or 32-bit MA_ID):
./filter/rastertop900escpr \
    --image photo.tif \
    --media Baryta \
    --input-slot FrontFineArt \
    --quality Max \
    --color-mode NoColorAdjustment \
    --user-media-id 5 \
    --print-duty -5 \
    --drying-time 2 \
    --bottom-edge-priority \
    --send 10.10.10.124:9100
```

---

## Base media reference table (`base_media` / `sn_id`)

| `sn_id` | `base_media` key | PPD ID | Black ink | Typical use for custom papers |
|---:|---|---:|:---:|---|
| `0` | `PlainPaper` | `0` | MK | Plain / proof sheets |
| `11` | `PremiumGlossy` | `13` | PK | High-gloss RC photo papers |
| `12` | `PremiumSemigloss` | `15` | PK | Satin / pearl / semigloss RC papers |
| `13` | `PremiumLuster` | `27` | PK | Luster RC photo papers (e.g. Example Luster 250) |
| `15` | `ArchivalMatte` | `14` | MK | Lightweight matte papers (< 200 g/m²) |
| `16` | `WatercolorRadiantWhite` | `17` | MK | Watercolor matte papers |
| `25` | `VelvetFineArt` | `53` | MK | Textured cotton fine-art papers (Hahnemühle Photo Rag) |
| `29` | `PhotoQualityInkJet` | `2` | MK | Coated inkjet matte papers |
| `39` | `UltraSmoothFineArt` | `88` | MK | Smooth cotton fine-art papers |
| `52` | `PhotoPaperGlossy` | `145` | PK | Standard glossy photo papers |
| `91` | `DiscLabel` | `26` | MK | CD/DVD standard surface |
| `92` | `DiscLabelPremium` | `76` | MK | CD/DVD premium surface |
| `142` | `ProofingWhiteSemimatte` | `106` | PK | Semimatte proofing papers |
| `169` | `CanvasMatte` | `155` | MK | Matte exhibition canvas |
| `171` | `CanvasSatin` | `153` | PK | Satin/glossy exhibition canvas |
| `174` | `Baryta` | `157` | PK | Baryta / fiber-based papers (Example Baryta Fibre) |
| `205` | `Washi` | `181` | MK | Japanese Kozo / Washi papers |
| `206` | `FineArtMatte` | `183` | MK | Heavyweight matte fine-art papers |
