# Custom Media & Third-Party Papers (`.emx` / NVRAM Slots `1..30`)

The Epson SureColor SC-P900 and SC-P700 support third-party papers (such as
Photolux, Hahnemühle, Canson, Ilford) and custom media configurations registered
into the printer's **30 non-volatile memory slots (`Slot 1..30`)** via SNMPv1 or
imported from `.emx` packages.

This driver (`rastertop900escpr`) supports custom media in two ways:
1. **Mode 2 — Hardware NVRAM Slots (`1..30`) & `.emx` Packages via the system media database:**
   **[`ep-media-installer` (`epmi`)](https://github.com/colorcube/ep-media-installer)**
   reads the printer's slots, fetches the matching `.emx` packages (with ICC
   profiles) and installs them system-wide with **"Für das System installieren"**.
   All registered papers then appear in the CUPS PPD (`*MediaType` +
   `*%cupsICCProfile`), in the CUPS filter and in **LumaPrint**.
2. **Mode 1 — Dynamic Per-Job Overrides (`lp -o ...` / CLI flags):**
   Allows passing `EpsonUserMediaID`, `EpsonPrintDuty` (`-50..+50%`),
   `EpsonDryingTime`, `EpsonBottomEdgePriority`, `EpsonRubbingReduction`, and
   `EpsonIgnorePanelMismatch` directly per print job without modifying the PPD.

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
   - `resolve_user_media_id()` in [`src/protocol.rs`](../src/protocol.rs)
     automatically resolves slot numbers `1..30` and UI IDs `1901..1930` to the
     32-bit `user_media_id` (`MA_ID`) from `custom_media.json`, or passes a
     32-bit `MA_ID` through directly.
3. **Extended Mechanical Flags (`ESC m seti`):** Configures `RubbingReduction`
   and `BottomEdgePriority` (print quality priority up to the bottom edge).
4. **Color Density / Print Duty (`ESC p setq` byte 5):** Signed 8-bit integer
   (`-50..+50` %) adjusting total ink limit (`InkDensity - 100` from `.emx`).
5. **Drying Time (`REMOTE1 DR`):** Drying time per printhead pass (`0..255`).
6. **Panel Mismatch Bypass (`ESC u chku` byte 0 = `1`):** Enabled by default so
   the printer starts immediately even if a different media type is currently
   selected on the front panel.

---

## System-wide installation (how Windows/macOS do it, and how this driver does it)

Epson's Media Installer writes the mechanical settings of a paper into one of the
printer's NVRAM slots. The **driver's paper list and ICC profiles, however, live on
each computer**: a local database is converted by the driver's "Convert Tool"
(`E_DCNV01.EXE` on Windows, *Convert Tool.app* on macOS) into the driver
configuration / PPD, and the profiles are installed into the system color folder.
Epson's FAQ: the Media Installer must run on every computer that prints.

On Linux the same split applies:

| What | Where | Written by |
|---|---|---|
| Mechanical settings, `MA_ID` | printer NVRAM slot `1..30` | `epmi` (SNMP) |
| Paper list (`custom_media.json`) | `/etc/epson-p900-escpr-cups/custom_media.json` | `epson-p900-media-install` |
| ICC profiles of custom papers | `/usr/local/share/color/icc/epson-p900-custom/` | `epson-p900-media-install` |
| PPD of each queue of this driver | `/etc/cups/ppd/<queue>.ppd` (via `lpadmin -P`, defaults kept) | `epson-p900-media-install` |
| PPDs for new queues | `/usr/lib/cups/driver/epson-p900-escpr` (`lpinfo -m`) | generated on the fly |

`epson-p900-media-install` (installed to `/usr/libexec/epson-p900-escpr-cups/`) is
started by `epmi` via `pkexec` (polkit action
`de.colorcube.epson-p900-escpr.install-media`, administrator password):

```bash
pkexec /usr/libexec/epson-p900-escpr-cups/epson-p900-media-install <staging-dir>
pkexec /usr/libexec/epson-p900-escpr-cups/epson-p900-media-install --remove <key>
sudo   /usr/libexec/epson-p900-escpr-cups/epson-p900-media-install --refresh-queues
```

The staging directory contains the complete paper list as `custom_media.json` and
the referenced profiles in `icc/` (see
[`examples/custom_media.example.json`](../examples/custom_media.example.json)).
The helper validates everything (keys, slots, base media, ICC signature, no
symlinks) before writing.

The filter reads `custom_media.json` from `$EPSON_P900_CUSTOM_MEDIA_JSON`
(development override), `/etc/epson-p900-escpr-cups/` and
`/usr/share/epson-p900-escpr-cups/`. Home directories are not searched: the
filter runs as user `lp`. **A job whose `MediaType` is neither built in nor
installed is aborted** with an error in the CUPS log instead of being printed
with plain-paper settings.

ICC profiles are taken from the absolute path in `"icc_profile"`, otherwise by
name from `$EPSON_P900_ICC_DIR`, `/usr/local/share/color/icc/epson-p900-custom/`,
`/usr/share/color/icc/epson-scp900/` and `/usr/share/color/icc/`.

---

## Managing NVRAM Slots & `.emx` Files (`ep-media-installer` / `epmi`)

Reading/writing the printer's 30 NVRAM slots over SNMPv1 and importing `.emx`
packages is handled by the standalone companion project
**[`ep-media-installer` (`epmi`)](https://github.com/colorcube/ep-media-installer)**.

Example of slots read from an SC-P900:

```text
Slot  UI ID  32-bit MA_ID  BaseSN  Gap                     Thick  Dry  Media Name (Printer LCD)
----------------------------------------------------------------------------------------------------------------
   1   1901     915000001      25  2.1 mm (Wide)          0.48mm    0  Example Fine Art Rag [25 (VelvetFineArt)]
   2   1902     915000002     174  1.5/1.6 mm (Standard)  0.30mm    0  Example Baryta Fibre [174 (Baryta)]
```

---

## Printing with Registered Custom Media or Dynamic Overrides

### Via CUPS (`lp`) using a synced slot key

```bash
lp -d Epson_SC_P900_ESCPR \
   -o PageSize=A4 \
   -o InputSlot=FrontFineArt \
   -o MediaType=Slot01_Example_Fine_Art_Rag \
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
   -o EpsonUserMediaID=1 \
   -o EpsonPrintDuty=-5 \
   -o EpsonDryingTime=2 \
   -o EpsonBottomEdgePriority=True \
   photo_profiled.tif
```

### Via CLI (`rastertop900escpr`)

```bash
# Using a synced slot key (automatically sets sn_id, 32-bit MA_ID, ICC profile, etc.):
./target/release/rastertop900escpr \
    --image photo.tif \
    --media Slot01_Example_Fine_Art_Rag \
    --input-slot FrontFineArt \
    --quality Max \
    --send 192.0.2.10:9100

# Or using on-the-fly parameters (--user-media-id accepts 1..30, 1901..1930, or 32-bit MA_ID):
./target/release/rastertop900escpr \
    --image photo.tif \
    --media Baryta \
    --input-slot FrontFineArt \
    --quality Max \
    --color-mode NoColorAdjustment \
    --user-media-id 5 \
    --print-duty -5 \
    --drying-time 2 \
    --bottom-edge-priority \
    --send 192.0.2.10:9100
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
