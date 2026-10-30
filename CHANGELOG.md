# Changelog

## Unreleased

### Added
- Advanced B&W Photo settings (`ESC q setb`, nine big-endian `i32`: toning wheel H/V, mid-gray tone as gamma × 100, brightness, contrast, shadow/highlight tonality, max optical density, highlight point shift), decoded from Epson Print Layout captures. New CUPS/PPD options `EpsonAbwTone` (Neutral/Cool/Warm), `EpsonAbwMidtone` (Light … Darkest, default Darker as in EPL), `EpsonAbwHighlightShift`, `EpsonAbwBrightness`, `EpsonAbwContrast`, `EpsonAbwShadowTone`, `EpsonAbwHighlightTone`, `EpsonAbwMaxDensity` (PPD lists steps of 5, any integer in range is accepted) and `EpsonAbwToneH`/`EpsonAbwToneV`; CLI `--abw KEY=VALUE`.

### Changed
- CLI (`--image`) in `AdvancedBW` mode converts the image to sRGB from its embedded profile, as EPL does; the firmware then converts to gray and tones. CUPS: send sRGB.
- Standalone CLI (`--image`): `EpsonAdobeRGB` and `EpsonStandard` now convert the image from its embedded ICC profile (sRGB if none) into the color space the printer LUT expects (Adobe RGB / sRGB), like Epson Print Layout does in "Auto" mode. Identity conversions are skipped so already matching data passes through bit-exact. Verified against the EPL reference captures (sRGB chart in Adobe RGB mode: mean deviation 0.52, Adobe RGB chart: identical).
- CUPS filter behavior is unchanged: the raster carries no source profile, so `EpsonAdobeRGB` / `EpsonStandard` expect data already in that color space (LumaPrint converts itself). The PPD labels now say so: `Printer Internal Adobe RGB (Input Must Be Adobe RGB)`, `Printer Internal sRGB (Input Must Be sRGB)`.

### Fixed
- `AdvancedBW` header now matches Epson Print Layout byte-for-byte (verified on six ABW captures): color byte 0 in `ESC q setq` / `ESC p setq` (was 3), LUT 7, `ESC q setb` after `ESC q seti` (was missing, so mid-gray tone and toning were never set), REMOTE1 `HD` with the ABW tags `0x12`–`0x1B` instead of the color tags.
- ICC mode (`EpsonICC`) now uses LittleCMS's default precalculation instead of `cmsFLAGS_HIGHRESPRECALC`, which reproduces Epson Print Layout bit-exactly: Perceptual + BPC and Relative Colorimetric + BPC on Photo Paper Glossy and Velvet Fine Art match the EPL captures on all 256 chart patches (previously up to 2–3 levels off on a few patches). EPL applies BPC to Perceptual as well (Velvet: up to 39 levels difference with/without), so Perceptual keeps BPC.
- CUPS filter: rasters with a resolution other than 720 × 720 dpi or with unsupported bits per pixel (e.g. 48 bpp / 16-bit RGB) now fail the job with a clear `InvalidData` error instead of printing at the wrong size or as a blank white page (`HWResolution` is read from header fields 5/6, bytes 276–283).
- Corrected the memory claim (`< 2 MB` peak RAM) in README and source docs: the uncompressed page is never held in memory, but the `RHV2`-compressed bands of one page are buffered because the `ESC t tsnd` preview must precede them; their size depends on the image.

## 0.2.0 – 2026-10-26 (beta)

Full native **Rust** port (`rastertop900escpr`) on `main`, verified end-to-end on SC-P900 hardware.

### Added
- Native Rust implementation of the CUPS filter and standalone CLI (`src/main.rs`, `src/rhv2.rs`, `src/icc.rs`, `src/protocol.rs`, `src/cups_raster.rs`, `src/ppd.rs`), replacing the Python/C prototype (preserved on the `python-prototype` branch).
- Band-wise 16-row streaming in CUPS filter mode (`< 2 MB` peak RAM even for A2 @ 720 dpi) including streaming generation of the 388 × 272 rotated LCD preview thumbnail (`ESC t tsnd`).
- Passive `custom_media.json` discovery for the printer's 30 NVRAM custom media slots (`ESC q setl` with 32-bit `MA_ID` from TLV Tag `0x24`, `ESC m seti` for `RubbingReduction` + `BottomEdgePriority`, `PrintDuty` in `ESC p setq`, `DryingTime` in `REMOTE1 DR`) and dynamic per-job overrides (`EpsonUserMediaID`, `EpsonPrintDuty`, `EpsonDryingTime`, `EpsonBottomEdgePriority`, `EpsonRubbingReduction`).
- `*%cupsICCProfile` metadata comments in generated PPDs and `cm-calibration=true` support for **LumaPrint** integration without triggering `colord` / `gstoraster` double-profiling.

### Fixed
- Prevented `colord` / `cfFilterGhostscript` (`gstoraster`) from applying an unwanted upstream ICC transformation (or `colord` default-profile fallback) before `rastertop900escpr`.
- Removed unused `DRI` (`0xFFDD`) marker from `ESC t tsnd` when the JPEG encoder does not emit `RSTm` markers, and fixed rotated thumbnail dimensions to `388 × 272` (`height = 272`).
- Prevented 1-pixel horizontal/vertical rounding differences in CUPS `ImageableArea` from triggering margin cropping in `run_cups_filter`.

## 0.1.0 – 2026-10-25 (python-prototype)

First working version, verified on an SC-P900.

### Added
- ESC/P-R job generation (`REMOTE1` setup, `setq/seti/setm/chku/setj/setq`,
  RHV2-compressed `bsn2` bands) with an independently written C compressor and
  Python fallback.
- Color management with Epson's per-paper ICC profiles via LittleCMS2; embedded
  source profiles (sRGB, Adobe RGB) are honoured.
- 18 media types, paper sources, paper sizes and per-media quality mapping from
  Epson's driver data.
- CUPS filter `rastertop900escpr` (verified end-to-end with CUPS Raster v3 + KDE/Qt print dialog), PPDs for SC-P900 and SC-P700, SELinux-aware install script.
- Job metadata (`JH` host, `JU` user, `HD` document/job name and print settings TLV) in `REMOTE1` plus `ESC t tsnd` JPEG page preview thumbnail so the SC-P900's 480 × 272 LCD touchscreen shows the full print preview, document name, and print settings while printing.
- Command line: test strip, image placement in mm (`--image-width-mm`,
  `--image-height-mm`, `--image-left-mm`, `--image-top-mm`, `--from-paper-edge`),
  direct sending (`--send`).
- Documentation in `docs/` (including `docs/rhv2-spec.md`), generated feature table
  (`scripts/generate_feature_table.py`).

### Fixed
- Printer asked for roll paper: wrong `REMOTE1` length fields, missing NUL
  prefix, malformed `SN`/`HD` in the job block.
- `setq` was packed as 10 instead of 9 bytes.
- Large jobs were aborted by a send timeout.
- Embedded ICC profile was lost when an image was scaled or placed.
- CUPS `pdftoraster` defaulted to 1-bit monochrome without explicit 24-bit RGB `ColorModel` in the PPD; fixed `cups_page_header2_t` uint32 block offset (`256`).
