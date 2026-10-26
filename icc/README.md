# Official Epson SC-P900 / SC-P700 ICC Profiles

This directory holds the **15 official Epson media ICC profiles** for the **SureColor SC-P900** and **SC-P700**.

## Why are `.icc` files excluded via `.gitignore`?
While the `epson-p900-escpr-cups` driver code is open-source under the **GNU General Public License v3.0 or later**, the factory ICC color profiles (`EE2NE_00.ICC` .. `EE2NE_14.ICC`) carry `Copyright (C) Seiko Epson Corporation. All rights reserved.` and therefore cannot be committed directly into a public Git repository.

Instead, the helper script [`fetch_icc_profiles.sh`](fetch_icc_profiles.sh) automatically downloads (or extracts from a local Windows `.exe` or macOS `.dmg` driver package) all 15 ICC profiles and verifies their SHA-256 checksums.

> [!NOTE]
> **Windows vs. macOS Installer:**
> We verified via SHA-256 that the 15 ICC profiles shipped in the Windows x64 installer (`EE2NE_00.ICC` .. `EE2NE_14.ICC`) and the macOS installer (`Epson_SC-P900_700_*.icc`) are **100% byte-for-byte identical**.
> By default, `fetch_icc_profiles.sh` prefers the Windows package (`SCP900_STD_WW_612200_W64.exe`) because it is **~41 MB** (compared to ~158 MB for the macOS `.dmg`) and only requires `7z` on Linux (no `xar`/`cpio` needed). However, the script supports both Windows `.exe` and macOS `.dmg` archives!

## Usage

```bash
# Option 1: Automatically download from Epson and extract into ./icc/
./icc/fetch_icc_profiles.sh

# Option 2: Extract from an already downloaded Windows .exe or macOS .dmg
./icc/fetch_icc_profiles.sh path/to/SCP900_STD_WW_612200_W64.exe
```

## Profile Mapping & SHA-256 Checksums

| Windows ID | Canonical Profile Filename | Media Name (UI) | Hardware `SN`/`MI` ID | Black Ink | SHA-256 (First 16 hex) |
| :--- | :--- | :--- | :---: | :---: | :--- |
| `EE2NE_00.ICC` | `Epson_SC-P900_700_ArchivalMattePaper.icc` | Epson Archival / Enhanced Matte | `15` (`0x0F`) | MK | `4cbeffd5d5ad4063` |
| `EE2NE_01.ICC` | `Epson_SC-P900_700_Baryta.icc` | Baryta | `174` (`0xAE`) | PK | `44a68d69c19829cb` |
| `EE2NE_02.ICC` | `Epson_SC-P900_700_CanvasMatte.icc` | Canvas Matte | `169` (`0xA9`) | MK | `b64b059cfc22e01a` |
| `EE2NE_03.ICC` | `Epson_SC-P900_700_CanvasSatin.icc` | Canvas Satin | `171` (`0xAB`) | PK | `fdff74f27312aa83` |
| `EE2NE_04.ICC` | `Epson_SC-P900_700_EpsonPremiumGlossy.icc` | Epson Premium Glossy | `11` (`0x0B`) | PK | `c52b0ee20f98b213` |
| `EE2NE_05.ICC` | `Epson_SC-P900_700_EpsonPremiumSemigloss.icc` | Epson Premium Semigloss | `12` (`0x0C`) | PK | `de92ba9224146d8a` |
| `EE2NE_06.ICC` | `Epson_SC-P900_700_FineArtMatte.icc` | Fine Art Matte | `206` (`0xCE`) | MK | `3b1927a90d375680` |
| `EE2NE_07.ICC` | `Epson_SC-P900_700_PhotoPaperGlossy.icc` | Photo Paper Glossy | `52` (`0x34`) | PK | `4d280a146c23ef3d` |
| `EE2NE_08.ICC` | `Epson_SC-P900_700_PremiumLusterPhotoPaper.icc` | Epson Premium Luster | `13` (`0x0D`) | PK | `32ed0fc271cfd9d4` |
| `EE2NE_09.ICC` | `Epson_SC-P900_700_Standard.icc` | Plain Paper / Photo Quality Ink Jet | `0` / `29` | MK | `c3f6e9893478e6aa` |
| `EE2NE_10.ICC` | `Epson_SC-P900_700_UltraSmoothFineArtPaper.icc` | Epson UltraSmooth Fine Art | `39` (`0x27`) | MK | `a22f03b31075cb86` |
| `EE2NE_11.ICC` | `Epson_SC-P900_700_VelvetFineArtPaper.icc` | Velvet Fine Art Paper | `25` (`0x19`) | MK | `99b1cee6b9304be3` |
| `EE2NE_12.ICC` | `Epson_SC-P900_700_Washi.icc` | Washi | `205` (`0xCD`) | MK | `14c1a73738314c26` |
| `EE2NE_13.ICC` | `Epson_SC-P900_700_WatercolorPaperRadiantWhite.icc` | Watercolor Paper - Radiant White | `16` (`0x10`) | MK | `28a7b590b5373e79` |
| `EE2NE_14.ICC` | `Epson_SC-P900_700_ProofingPaperWhiteSemimatte.icc` | Proofing Paper White Semimatte | `142` (`0x8E`) | PK | `8a0564bec3dee8b0` |
