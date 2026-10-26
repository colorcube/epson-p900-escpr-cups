# epson-p900-escpr-cups

A native **Rust** Linux CUPS driver (`rastertop900escpr`) for the **Epson SureColor
SC-P900** (and, untested, the **SC-P700**) that drives the printer the same way
Epson's official Windows and macOS drivers do: color management with Epson's
paper-specific ICC profiles via `liblcms2` on the host, band-wise streaming
(`RHV2` compression with `< 2 MB` peak RAM even on A2 at 720 dpi), and exact
media, NVRAM custom slot (`ESC q setl`), and quality settings sent to the
printer.

> [!WARNING]
> **Beta / experimental.** Verified on a real SC-P900 so far: plain paper and
> Epson Photo Paper Glossy from the rear sheet feeder, A4, quality "High",
> Epson ICC color management, plus custom media NVRAM slot discovery. Everything
> else is implemented from Epson's driver data but has not been printed yet – see
> [docs/supported-features.md](docs/supported-features.md).

## Results

The same image printed with the official Windows driver (Epson Print Layout)
and with this driver on Epson Photo Paper Glossy: **no visible difference, also
in daylight.** The same image printed driverless over IPP showed a small
difference (slightly lighter, less saturated colors) – details in
[docs/why-not-ipp.md](docs/why-not-ipp.md).

## Do I need this driver?

| | IPP / driverless (built into Linux) | **this driver (`rastertop900escpr`)** | Gutenprint |
|---|---|---|---|
| Color management | printer firmware, sRGB / Adobe RGB | Epson ICC profile per paper on the computer (`liblcms2`) | own, on the computer |
| Media / quality settings | 16 media types, 3 quality levels | all 18 Epson media types + 30 custom NVRAM slots, Carbon Black, Overcoat | own |
| Typical use | documents, everyday photos | photo and fine-art printing like on Windows/macOS | direct per-nozzle control (SC-P900 not supported by Gutenprint) |

Short version: for documents IPP is fine. For prints that should look like
they came from the Epson driver, use this one. Why 720 dpi data is enough
although the printer advertises 5760 × 1440 dpi is explained in
[docs/resolution.md](docs/resolution.md), and why `ESC/P-R` is used instead of
Gutenprint is explained in [docs/why-not-gutenprint.md](docs/why-not-gutenprint.md).

## Companion Projects

- **[`ep-media-installer` (`epmi`)](../ep-media-installer)**: Standalone Rust CLI &
  GUI tool for managing the printer's 30 custom media NVRAM slots (`1..30`) over
  SNMP, importing/downloading `.emx` media packages, and writing
  `custom_media.json` for this CUPS driver.
- **[`LumaPrint`](../LumaPrint)**: Rust/egui fine-art photo layout and soft-proofing
  application for Linux that works with any CUPS printer and automatically reads
  the paper-to-ICC mappings from this driver's PPD.
- *(Note: The original Python prototype of the driver and media installer is
  preserved on the `python-prototype` Git branch.)*

## Requirements

- CUPS (`libcupsfilters` / `cupstestppd`)
- Rust toolchain (`cargo` / `rustc`)
- LittleCMS 2 development library (`liblcms2-dev` on Debian/Ubuntu, `lcms2-devel` on Fedora/openSUSE)
- `7z` (`p7zip`) to extract the Epson ICC profiles from the official driver package
- The printer reachable over the network (raw port 9100) or USB

## Installation

```bash
# 1. Fetch Epson's ICC profiles from the official driver package
#    (they may not be redistributed, so they are not part of this repository)
./icc/fetch_icc_profiles.sh

# 2. Option A: Install driver files into CUPS ONLY (without creating a queue)
#    Use this if you want to add the printer manually via KDE / GNOME / CUPS UI:
sudo ./install.sh

# 2. Option B: Install driver files AND register the CUPS queue automatically:
sudo ./install.sh 192.0.2.10
```

This installs the single compiled binary `/usr/lib/cups/filter/rastertop900escpr`,
`custom_media.json` to `/usr/share/epson-p900-escpr-cups/`, the PPDs to
`/usr/share/cups/model/epson-p900-escpr/`, and the ICC profiles to
`/usr/share/color/icc/epson-scp900/`.

### Adding the printer manually in KDE / GNOME / CUPS Web UI (`http://localhost:631`)

If you ran `sudo ./install.sh` without an IP address (or when connecting via USB):
1. **Connection / Device URI:**
   - **Network (Auto-Discovered or Manual):** Select the discovered **EPSON SC-P900 Series** (`dnssd://EPSON%20SC-P900%20Series._ipp._tcp.local/?uuid=...` or `lpd://.../PASSTHRU`), or choose **AppSocket / HP JetDirect** and enter `socket://<PRINTER_IP>:9100` (e.g. `socket://192.0.2.10:9100`).
   - **USB:** Select the USB-connected **EPSON SC-P900 Series** (`usb://EPSON/SC-P900%20Series...`).
2. **Driver Selection:**
   - **Recommended Drivers screen (KDE / system-config-printer):** Select the ★ **`PPD-Datei` (`epson-p900-escpr/Epson-SC-P900-ESCPR.ppd`)** entry (as opposed to `Treiberloses Drucken` / `driverless`).
   - **Manual Driver Search (`Manuelle Treibersuche…`):** Under Manufacturer **EPSON**, our driver **`EPSON SC-P900 Series (SureColor) - Official ICC + ESC/P-R Lv5`** is automatically pre-selected.
   - *(Note: If KDE System Settings was already open while running `sudo ./install.sh`, close and reopen System Settings so it reloads the CUPS PPD list.)*

## Usage

### From applications / CUPS

Select the `Epson_SC_P900_ESCPR` queue and choose paper type, paper source and
quality in your application's print dialog (e.g. under *Advanced* in KDE/Qt or
GTK print dialogs), or print from the command line:

```bash
lp -d Epson_SC_P900_ESCPR \
   -o PageSize=A4 -o InputSlot=Rear \
   -o MediaType=PhotoPaperGlossy -o PrintQuality=High \
   photo.jpg
```

The available values are listed in
[docs/supported-features.md](docs/supported-features.md).

### Command line without CUPS

```bash
# Ink-saving test strip (10 color patches, 1.8 × 1.8 cm each) on A4 plain paper
./target/release/rastertop900escpr --test-strip --media PlainPaper -o strip.prn

# Print an image 148.7 mm wide, 30.6 mm from the left and 20 mm from the top
# paper edge, and send it straight to the printer
./target/release/rastertop900escpr --image photo.jpg \
    --media PhotoPaperGlossy --input-slot Rear --page-size A4 --quality High \
    --image-width-mm 148.7 --image-left-mm 30.6 --image-top-mm 20 --from-paper-edge \
    --send 192.0.2.10:9100
```

Run `./target/release/rastertop900escpr --help` for all options.

## Color Management Workflows

1. **Standard CUPS applications (PDF viewers, browsers, office apps):**
   - Leave `Color Management Mode` at its default (`EpsonICC` / *Epson Official ICC Profile*).
   - `rastertop900escpr` transforms the incoming 720 dpi sRGB CUPS raster into the official Epson ICC profile for the selected `MediaType` using `liblcms2` (`RelativeBPC` + Black Point Compensation by default) and instructs the printer firmware to disable internal color adjustment (`LUT = 4`).
2. **Wide-gamut photo workflows (`LumaPrint`, darktable, GIMP, Krita, or `--image` CLI):**
   - Because standard CUPS PDF-to-raster filters (`gstoraster` / `pdftoraster`) rasterize pages into 8-bit `sRGB`, wide-gamut photo applications (working in **Adobe RGB** or **ProPhoto RGB**) perform the ICC conversion (`Source ICC -> Epson Paper ICC`) themselves so they can provide soft-proofing and preserve the printer's full color gamut.
   - When the application already converts to the paper profile, pass `-o EpsonColorMode=NoColorAdjustment` (or `-o cm-calibration=true`) so `rastertop900escpr` forwards the pre-profiled device RGB values 1:1.
   - In standalone CLI mode (`--image photo.tif`), `rastertop900escpr` reads the embedded source ICC profile (e.g. Adobe RGB or ProPhoto RGB) directly from the image file and transforms it to the Epson paper ICC profile in a single step without any sRGB intermediate.
   - Note: The PPD stores paper-to-profile mappings as `*%cupsICCProfile` comments so companion tools like **LumaPrint** can auto-select the matching paper profile while preventing CUPS `colord` / `gstoraster` from applying an unwanted second ICC pass upstream.

## Documentation

| Document | Content |
|---|---|
| [docs/supported-features.md](docs/supported-features.md) | Media, paper sources, sizes, quality levels – with test status |
| [docs/custom-media.md](docs/custom-media.md) | Custom media & third-party papers (`.emx`, NVRAM slots `1..30`, `custom_media.json`, `ep-media-installer`, dynamic `lp -o` flags) |
| [docs/why-not-ipp.md](docs/why-not-ipp.md) | Difference to driverless/IPP printing, side-by-side test |
| [docs/why-not-gutenprint.md](docs/why-not-gutenprint.md) | Why `ESC/P-R Level 5` instead of a classic Gutenprint (`ESC i`) driver on the SC-P900 |
| [docs/resolution.md](docs/resolution.md) | Image resolution vs. printer resolution, halftoning |
| [docs/troubleshooting.md](docs/troubleshooting.md) | Common problems (including the harmless CUPS `lpadmin` PPD warning) |
| [docs/protocol.md](docs/protocol.md) | Byte-level protocol notes for contributors |
| [docs/rhv2-spec.md](docs/rhv2-spec.md) | Formal bitstream specification of the `RHV2` planar band format |
| [icc/README.md](icc/README.md) | Where the ICC profiles come from |
| [CHANGELOG.md](CHANGELOG.md) | Changes |

## How it works

```mermaid
flowchart LR
    A["CUPS raster stream or image file"] --> B["16-row band streaming + LittleCMS2 ICC conversion"]
    B --> C["720 dpi RGB bands, RHV2 compression"]
    C --> D["REMOTE1 setup + ESC/P-R Level 5 job"]
    D --> E["SC-P900: color adjustment off, firmware halftoning"]
```

## Contributing

Test reports are the most valuable contribution right now: which paper, source,
size and quality you printed, and whether it worked. If the printer shows an
error, attach the job written with `-o job.prn`.

## Legal

- Code: GNU General Public License v3.0 or later, see [LICENSE](LICENSE).
- Independent open-source implementation developed for hardware interoperability
  (EU Directive 2009/24/EC, Art. 6): protocol packet structures, media IDs, and
  band compression were determined through interoperability analysis of print
  data streams and publicly distributed PPD / configuration tables. No
  proprietary code, binaries, or ICC profiles are included in this repository;
  the ICC profiles are extracted locally from Epson's official driver package by
  the user.
- Epson and SureColor are trademarks of Seiko Epson Corporation. This project is
  not affiliated with or endorsed by Epson.
