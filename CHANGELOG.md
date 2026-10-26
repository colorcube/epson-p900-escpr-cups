# Changelog

## 0.1.0 – unreleased (beta)

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
