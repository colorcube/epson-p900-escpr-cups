# Custom Media & Third-Party Papers (`.emx` / NVRAM Slots)

The Epson SureColor SC-P900 and SC-P700 support third-party papers (such as
Photolux, Hahnemühle, Canson, Ilford) and custom media configurations registered
via **Epson Media Installer** into the printer's 30 non-volatile memory slots
(`Slot 1..30` / driver IDs `1901..1930`).

This driver supports custom media in two ways:
1. **Dynamic per-job options (`lp -o ...` / CLI flags)** – works immediately
   without modifying the PPD or requiring `sudo`. Ideal for dedicated photo
   printing applications such as **LumaPrint**.
2. **Persistent registration via `custom_media.json`** – makes custom papers
   selectable in `rastertop900escpr` by name and (after regenerating the PPD)
   visible in any standard Linux print dialog (KDE/Qt, GTK, GIMP, Darktable)
   with automatic `*cupsICCProfile` association.

---

## How the SC-P900 handles custom media internally

When printing on a custom paper, the SC-P900 firmware expects **both** a base
media reference and the custom parameters in the ESC/P-R Level 5 header:

1. **Base Media (`ESC q setq` byte 0 = `sn_id`):** Must still be set to the
   underlying Epson reference media (for example `25` = *Velvet Fine Art Paper*,
   `174` = *Epson Baryta*, or `52` = *Photo Paper Glossy*). The firmware uses
   this base ID to select **Photo Black (PK) vs. Matte Black (MK)** and the
   internal halftoning LUT.
2. **Custom NVRAM Slot (`ESC q setl`):** Sends the 32-bit big-endian
   `UserMediaTypeID` (`1..30` or `1901..1930`) right after `ESC u chku`. This
   instructs the printer to load the mechanical parameters stored in that slot
   (platen gap, paper thickness, feed roller tension, paper feed offset, skew
   check).
3. **Extended Mechanical Flags (`ESC m seti`):** Configures `RubbingReduction`
   and `BottomEdgePriority` (print quality priority up to the bottom edge).
4. **Color Density / Print Duty (`ESC p setq` byte 5):** Signed 8-bit integer
   (`-50..+50` %) adjusting total ink limit / color density as specified in an
   `.emx` media profile.
5. **Drying Time (`REMOTE1 DR`):** Drying time per printhead pass (`0..255`).
6. **Panel Mismatch Bypass (`ESC u chku` byte 0 = `1`):** Enabled by default so
   the printer starts immediately even if a different media type is currently
   shown on the front panel.

---

## 1. Dynamic per-job usage (no PPD rebuild required)

Applications (like **LumaPrint**) that perform their own 16-bit ICC color
conversion or manage `.emx` packages directly can pass custom media parameters
on the fly for any job:

### Via CUPS (`lp`)

```bash
lp -d Epson_SC_P900_ESCPR \
   -o PageSize=A4 \
   -o InputSlot=FrontFineArt \
   -o MediaType=Baryta \
   -o PrintQuality=Max \
   -o EpsonColorMode=NoColorAdjustment \
   -o UserMediaID=1901 \
   -o PrintDuty=-5 \
   -o DryingTime=2 \
   -o BottomEdgePriority=true \
   photo_profiled.tif
```

Supported CUPS options:
- `MediaType=<BaseKey>`: Base Epson media key (determines PK/MK and base LUT; see table below).
- `UserMediaID=<1..30 | 1901..1930>` (alias: `CustomMediaSlot`): NVRAM slot in the printer (`0` = omit `ESC q setl`).
- `PrintDuty=<-50..50>` (alias: `ColorDensity`): Ink density adjustment in percent.
- `DryingTime=<0..255>`: Drying time per pass.
- `BottomEdgePriority=true|false`: Bottom edge print quality priority.
- `RubbingReduction=<0..65535>`: Rubbing reduction flag.
- `EpsonColorMode=NoColorAdjustment` (or `cm-calibration=true`): Disables host ICC conversion in the filter when the calling application has already applied the paper's ICC profile.

### Via CLI (`rastertop900escpr`)

```bash
./filter/rastertop900escpr \
    --image photo.tif \
    --media Baryta \
    --input-slot FrontFineArt \
    --quality Max \
    --color-mode NoColorAdjustment \
    --user-media-id 1901 \
    --print-duty -5 \
    --drying-time 2 \
    --bottom-edge-priority \
    --send 10.10.10.124:9100
```

---

## 2. Persistent registration via `custom_media.json`

To register custom media permanently for the driver and optionally include them
in the CUPS PPD for all Linux applications, create a `custom_media.json` file in
one of the following paths (checked in this order):

1. `$EPSON_P900_CUSTOM_MEDIA_JSON` (environment variable)
2. `/etc/epson-p900-escpr-cups/custom_media.json`
3. `/usr/share/epson-p900-escpr-cups/custom_media.json`
4. `<repo-root>/custom_media.json`

### Example `custom_media.json`

```json
[
  {
    "key": "CustomSlot01_PhotoluxBaryta",
    "name": "Photolux Studio Baryta 310 (Slot 1)",
    "base_media": "Baryta",
    "user_media_id": 1901,
    "icc_profile": "/usr/share/color/icc/epson-scp900/Photolux_Studio_Baryta_310.icc",
    "print_duty": -5,
    "drying_time": 2,
    "bottom_edge_priority": false,
    "rubbing_reduction": 0
  },
  {
    "key": "CustomSlot02_HahnemuehlePhotoRag",
    "name": "Hahnemühle Photo Rag 308 (Slot 2)",
    "base_media": "VelvetFineArt",
    "user_media_id": 1902,
    "icc_profile": "/usr/share/color/icc/epson-scp900/HFA_EpsSCP900_MK_PhotoRag308.icc",
    "print_duty": 0,
    "drying_time": 0,
    "bottom_edge_priority": true,
    "rubbing_reduction": 0
  }
]
```

Once listed in `custom_media.json`:
- **Immediate availability in the filter:** `rastertop900escpr` immediately
  recognizes `--media CustomSlot01_PhotoluxBaryta` (or `-o MediaType=CustomSlot01_PhotoluxBaryta`)
  and automatically applies its `icc_profile` (when `EpsonColorMode=EpsonICC`),
  `user_media_id`, `print_duty`, `drying_time`, and mechanical settings.
- **Updating the CUPS PPD for standard print dialogs:** Run `./install.sh` and
  `sudo ./install.sh <printer-ip>` (or `python3 scripts/generate_ppd.py`). This
  adds each custom media entry to the PPD's `MediaType` list and emits a matching
  `*cupsICCProfile RGB.<key>./<name>: "<icc_profile>"` line.

---

## Base media reference table (`base_media` / `sn_id`)

When mapping an `.emx` file or printer NVRAM slot to `base_media`, use either
the `MediaType` key or look up its numeric Epson `sn_id`:

| `sn_id` | `base_media` key | Black ink | Typical use for custom papers |
|---:|---|:---:|---|
| `0` | `PlainPaper` | MK | Plain / proof sheets |
| `11` | `PremiumGlossy` | PK | High-gloss RC photo papers |
| `12` | `PremiumSemigloss` | PK | Satin / pearl / semigloss RC papers |
| `15` | `ArchivalMatte` | MK | Lightweight matte papers (< 200 g/m²) |
| `25` | `VelvetFineArt` | MK | Textured cotton fine-art papers (Photo Rag, Museum Etching) |
| `27` | `PremiumLuster` | PK | Luster RC photo papers |
| `31` | `WatercolorRadiantWhite` | MK | Watercolor matte papers |
| `52` | `PhotoPaperGlossy` | PK | Standard glossy photo papers |
| `58` | `UltraSmoothFineArt` | MK | Smooth cotton fine-art papers |
| `172` | `ValueGlossy` | PK | Thin glossy photo papers |
| `174` | `Baryta` | PK | Baryta / fiber-based glossy & satin papers |
| `175` | `FineArtMatte` | MK | Heavy matte fine-art papers |
| `176` | `PhotoThick` | PK | Heavyweight glossy/luster board |
| `177` | `Washi` | MK | Japanese Washi papers |
| `178` | `CanvasSatin` | PK | Satin/glossy canvas |
| `179` | `CanvasMatte` | MK | Matte canvas |
