#!/usr/bin/env python3
"""
escpr_protocol.py - Independent implementation of the Epson REMOTE1 + ESC/P-R
                    Level 5 (ESCPR) protocol generator for the Epson SureColor
                    SC-P900 and SC-P700 (reverse engineered for interoperability).

Drives the printer identically to the official Windows/macOS driver:
  1. Sends REMOTE1 initialization (TI, DP, SN, US, DR, PP paper feed path).
  2. Enters ESC/P-R Level 5 mode (ESC ( R 06 00 00 ESCPR).
  3. Sends hardware media/quality configuration:
     - ESC q setq (Hardware MediaTypeID, PrintQuality 1..4 or 7=Carbon Black)
     - ESC q seti (CompressMode=1 RHV2, OverCoat 1=Off / 2=Black Enhance Overcoat)
     - ESC m setm (MediaSizeID, Borderless, SkipBlankPage, Unidirectional)
     - ESC u chku (NonCheckPrintMode)
     - ESC j setj (720 dpi page & printable area geometry)
     - ESC p sttp (Start page)
     - ESC p setq (LUT=4 'No Color Adjustment / Host ICC Mode', Gamma=220)
  4. Encodes 720x720 dpi RGB image bands in 16-line ESC d bsn2 packets using
     RHV2 compression (C accelerator librhv2.so with pure-Python fallback).
  5. Terminates page & job cleanly (ESC p endp, ESC j endj, REMOTE1 LD + JE).

SPDX-License-Identifier: GPL-3.0-or-later
"""

from __future__ import annotations

import ctypes
import datetime
import json
import os
import pathlib
import subprocess
import struct
from dataclasses import dataclass
from typing import Dict, Iterable, List, Optional, Tuple


@dataclass(frozen=True)
class MediaSpec:
    key: str
    name: str
    ppd_id: int
    sn_id: int              # Hardware base MediaTypeID in ESC q setq byte 0
    icc_profile: str        # Official or custom ICC filename
    win_icc: str            # Official Windows ICC filename (EE2NE_00..14.ICC)
    is_matte: bool          # Uses Matte Black (MK) vs Photo Black (PK)
    default_overcoat: int   # 2 = Black Enhance Overcoat ON, 1 = OFF
    supports_carbon_black: bool
    user_media_id: int = 0          # >0 (32-bit MA_ID in NVRAM slot Tag 0x24, or 1901..1930) emits ESC q setl
    print_duty: int = 0             # -50..+50 ink duty in ESC p setq byte 5
    drying_time: int = 0            # 0..10 pass drying time in REMOTE1 DR
    bottom_edge_priority: bool = False
    rubbing_reduction: int = 0
    slot: int = 0                   # Printer NVRAM slot 1..30 (0 = built-in or unslotted)
    platen_gap: int = 0             # Tag 0x22 in NVRAM slot (0=1.6, 1=2.1, 2=2.4, 3=0.95, 4=1.35)
    thickness_01mm: int = 0         # Tag 0x19 in NVRAM slot (units of 0.01 mm, e.g. 27 = 0.27 mm)
    cut_sheet_feed_offset: int = 0  # Tag 0x26 in NVRAM slot (-300..+300)
    roll_paper_feed_offset: int = 0 # Tag 0x25 in NVRAM slot (-300..+300)


# Complete table of all 18 official Epson SC-P900 / SC-P700 media types
MEDIA_SPECS: List[MediaSpec] = [
    MediaSpec(
        key="PremiumGlossy",
        name="Epson Premium Glossy",
        ppd_id=13,
        sn_id=11,
        icc_profile="Epson_SC-P900_700_EpsonPremiumGlossy.icc",
        win_icc="EE2NE_04.ICC",
        is_matte=False,
        default_overcoat=2,
        supports_carbon_black=True,
    ),
    MediaSpec(
        key="PremiumSemigloss",
        name="Epson Premium Semigloss",
        ppd_id=15,
        sn_id=12,
        icc_profile="Epson_SC-P900_700_EpsonPremiumSemigloss.icc",
        win_icc="EE2NE_05.ICC",
        is_matte=False,
        default_overcoat=2,
        supports_carbon_black=True,
    ),
    MediaSpec(
        key="PremiumLuster",
        name="Epson Premium Luster Photo Paper",
        ppd_id=27,
        sn_id=13,
        icc_profile="Epson_SC-P900_700_PremiumLusterPhotoPaper.icc",
        win_icc="EE2NE_08.ICC",
        is_matte=False,
        default_overcoat=2,
        supports_carbon_black=True,
    ),
    MediaSpec(
        key="PhotoPaperGlossy",
        name="Photo Paper Glossy",
        ppd_id=145,
        sn_id=52,
        icc_profile="Epson_SC-P900_700_PhotoPaperGlossy.icc",
        win_icc="EE2NE_07.ICC",
        is_matte=False,
        default_overcoat=2,
        supports_carbon_black=True,
    ),
    MediaSpec(
        key="Baryta",
        name="Epson Traditional Photo Paper / Baryta",
        ppd_id=157,
        sn_id=174,
        icc_profile="Epson_SC-P900_700_Baryta.icc",
        win_icc="EE2NE_01.ICC",
        is_matte=False,
        default_overcoat=2,
        supports_carbon_black=True,
    ),
    MediaSpec(
        key="VelvetFineArt",
        name="Velvet Fine Art Paper",
        ppd_id=53,
        sn_id=25,
        icc_profile="Epson_SC-P900_700_VelvetFineArtPaper.icc",
        win_icc="EE2NE_11.ICC",
        is_matte=True,
        default_overcoat=1,
        supports_carbon_black=False,
    ),
    MediaSpec(
        key="UltraSmoothFineArt",
        name="Epson UltraSmooth Fine Art Paper",
        ppd_id=88,
        sn_id=39,
        icc_profile="Epson_SC-P900_700_UltraSmoothFineArtPaper.icc",
        win_icc="EE2NE_10.ICC",
        is_matte=True,
        default_overcoat=1,
        supports_carbon_black=False,
    ),
    MediaSpec(
        key="WatercolorRadiantWhite",
        name="Watercolor Paper Radiant White",
        ppd_id=17,
        sn_id=16,
        icc_profile="Epson_SC-P900_700_WatercolorPaperRadiantWhite.icc",
        win_icc="EE2NE_13.ICC",
        is_matte=True,
        default_overcoat=1,
        supports_carbon_black=False,
    ),
    MediaSpec(
        key="FineArtMatte",
        name="Signature Worthy / Fine Art Matte",
        ppd_id=183,
        sn_id=206,
        icc_profile="Epson_SC-P900_700_FineArtMatte.icc",
        win_icc="EE2NE_06.ICC",
        is_matte=True,
        default_overcoat=1,
        supports_carbon_black=False,
    ),
    MediaSpec(
        key="Washi",
        name="Japanese Kozo Paper Thin / Washi",
        ppd_id=181,
        sn_id=205,
        icc_profile="Epson_SC-P900_700_Washi.icc",
        win_icc="EE2NE_12.ICC",
        is_matte=True,
        default_overcoat=1,
        supports_carbon_black=False,
    ),
    MediaSpec(
        key="ArchivalMatte",
        name="Epson Archival / Enhanced Matte Paper",
        ppd_id=14,
        sn_id=15,
        icc_profile="Epson_SC-P900_700_ArchivalMattePaper.icc",
        win_icc="EE2NE_00.ICC",
        is_matte=True,
        default_overcoat=1,
        supports_carbon_black=False,
    ),
    MediaSpec(
        key="PhotoQualityInkJet",
        name="Epson Photo Quality Ink Jet",
        ppd_id=2,
        sn_id=29,
        icc_profile="Epson_SC-P900_700_ArchivalMattePaper.icc",
        win_icc="EE2NE_00.ICC",
        is_matte=True,
        default_overcoat=1,
        supports_carbon_black=False,
    ),
    MediaSpec(
        key="CanvasSatin",
        name="Exhibition Canvas Satin",
        ppd_id=153,
        sn_id=171,
        icc_profile="Epson_SC-P900_700_CanvasSatin.icc",
        win_icc="EE2NE_03.ICC",
        is_matte=False,
        default_overcoat=1,
        supports_carbon_black=False,
    ),
    MediaSpec(
        key="CanvasMatte",
        name="Exhibition Canvas Matte",
        ppd_id=155,
        sn_id=169,
        icc_profile="Epson_SC-P900_700_CanvasMatte.icc",
        win_icc="EE2NE_02.ICC",
        is_matte=True,
        default_overcoat=1,
        supports_carbon_black=False,
    ),
    MediaSpec(
        key="PlainPaper",
        name="Plain Paper / Bright White Paper",
        ppd_id=0,
        sn_id=0,
        icc_profile="Epson_SC-P900_700_Standard.icc",
        win_icc="EE2NE_09.ICC",
        is_matte=True,
        default_overcoat=1,
        supports_carbon_black=False,
    ),
    MediaSpec(
        key="ProofingWhiteSemimatte",
        name="Epson Proofing Paper White Semimatte",
        ppd_id=106,
        sn_id=142,
        icc_profile="Epson_SC-P900_700_ProofingPaperWhiteSemimatte.icc",
        win_icc="EE2NE_14.ICC",
        is_matte=False,
        default_overcoat=1,
        supports_carbon_black=False,
    ),
    MediaSpec(
        key="DiscLabel",
        name="CD/DVD Disc Label",
        ppd_id=26,
        sn_id=91,
        icc_profile="Epson_SC-P900_700_Standard.icc",
        win_icc="EE2NE_09.ICC",
        is_matte=True,
        default_overcoat=1,
        supports_carbon_black=False,
    ),
    MediaSpec(
        key="DiscLabelPremium",
        name="CD/DVD Premium Surface",
        ppd_id=76,
        sn_id=92,
        icc_profile="Epson_SC-P900_700_Standard.icc",
        win_icc="EE2NE_09.ICC",
        is_matte=True,
        default_overcoat=1,
        supports_carbon_black=False,
    ),
]

MEDIA_BY_KEY: Dict[str, MediaSpec] = {}
for _m in MEDIA_SPECS:
    MEDIA_BY_KEY[_m.key.lower()] = _m
    MEDIA_BY_KEY[str(_m.ppd_id)] = _m
    MEDIA_BY_KEY[f"sn_{_m.sn_id}"] = _m


def load_custom_media_specs(extra_json_path: Optional[pathlib.Path] = None) -> None:
    """
    Load optional custom media entries (imported from .emx or read from the
    printer's 30 NVRAM slots 1..30) from custom_media.json.
    """
    here = pathlib.Path(__file__).resolve().parent
    candidates: List[pathlib.Path] = []
    if extra_json_path is not None:
        candidates.append(pathlib.Path(extra_json_path))
    env_json = os.environ.get("EPSON_P900_CUSTOM_MEDIA_JSON")
    if env_json:
        candidates.append(pathlib.Path(env_json))
    candidates.extend([
        here.parent / "custom_media.json",
        pathlib.Path("/etc/epson-p900-escpr-cups/custom_media.json"),
        pathlib.Path("/usr/share/epson-p900-escpr-cups/custom_media.json"),
        pathlib.Path.home() / ".config" / "epson-p900-escpr" / "custom_media.json",
        pathlib.Path.home() / ".config" / "epson-p900" / "custom_media.json",
    ])
    seen_keys = {m.key.lower() for m in MEDIA_SPECS}
    for path in candidates:
        if not path.is_file():
            continue
        try:
            entries = json.loads(path.read_text(encoding="utf-8"))
        except Exception:
            continue
        if not isinstance(entries, list):
            continue
        for item in entries:
            if not isinstance(item, dict):
                continue
            slot = int(item.get("slot", 0) or 0)
            user_media_id = int(item.get("user_media_id", 1900 + slot if 1 <= slot <= 30 else 0))
            key = str(item.get("key") or (f"CustomSlot{slot}" if slot else "")).strip()
            if not key or key.lower() in seen_keys:
                continue
            base_ref = str(item.get("base_media") or item.get("base_sn_id") or "VelvetFineArt")
            base = MEDIA_BY_KEY.get(
                base_ref.lower(),
                MEDIA_BY_KEY.get(f"sn_{base_ref}", MEDIA_BY_KEY["velvetfineart"]),
            )
            thickness_01mm = int(item.get("thickness_01mm", 0) or 0)
            if not thickness_01mm and item.get("thickness_mm") is not None:
                thickness_01mm = int(round(float(item["thickness_mm"]) * 100.0))
            spec = MediaSpec(
                key=key,
                name=str(item.get("name") or key),
                ppd_id=int(item.get("base_ppd_id", base.ppd_id)),
                sn_id=int(item.get("base_sn_id", base.sn_id)),
                icc_profile=str(item.get("icc_profile") or base.icc_profile),
                win_icc=str(item.get("win_icc") or base.win_icc),
                is_matte=bool(item.get("is_matte", base.is_matte)),
                default_overcoat=int(item.get("default_overcoat", base.default_overcoat)),
                supports_carbon_black=bool(
                    item.get("supports_carbon_black", base.supports_carbon_black)
                ),
                user_media_id=user_media_id,
                print_duty=int(item.get("print_duty", 0)),
                drying_time=int(item.get("drying_time", 0)),
                bottom_edge_priority=bool(item.get("bottom_edge_priority", False)),
                rubbing_reduction=int(item.get("rubbing_reduction", 0)),
                slot=slot,
                platen_gap=int(item.get("platen_gap", 0) or 0),
                thickness_01mm=thickness_01mm,
                cut_sheet_feed_offset=int(item.get("cut_sheet_feed_offset", 0) or 0),
                roll_paper_feed_offset=int(item.get("roll_paper_feed_offset", 0) or 0),
            )
            MEDIA_SPECS.append(spec)
            MEDIA_BY_KEY[spec.key.lower()] = spec
            if 1 <= slot <= 30:
                MEDIA_BY_KEY[f"slot_{slot}"] = spec
                MEDIA_BY_KEY[f"customslot{slot:02d}"] = spec
                MEDIA_BY_KEY[str(1900 + slot)] = spec
            if user_media_id:
                MEDIA_BY_KEY[str(user_media_id)] = spec
            seen_keys.add(spec.key.lower())


load_custom_media_specs()


def resolve_user_media_id(raw_id: int, media: MediaSpec) -> int:
    """
    Resolve a requested custom media identifier to the 32-bit MA_ID (Tag 0x24
    in the printer's NVRAM slot) required by 'ESC q setl'.
    Accepts:
      - 0: uses media.user_media_id
      - 1..30: looks up NVRAM slot 1..30 in MEDIA_BY_KEY['slot_<N>'] if loaded
      - 1901..1930: macOS/Windows driver UI slot IDs -> looks up NVRAM slot N-1900
      - 32-bit MA_ID (e.g. 915907607): passed through directly
    """
    uid = int(raw_id) if int(raw_id) > 0 else int(media.user_media_id)
    if 1 <= uid <= 30:
        slot_spec = MEDIA_BY_KEY.get(f"slot_{uid}")
        if slot_spec is not None and slot_spec.user_media_id > 0:
            return slot_spec.user_media_id
    elif 1901 <= uid <= 1930:
        slot_spec = MEDIA_BY_KEY.get(f"slot_{uid - 1900}")
        if slot_spec is not None and slot_spec.user_media_id > 0:
            return slot_spec.user_media_id
    return uid


def get_media_spec(media_option: str) -> MediaSpec:
    key = (media_option or "PlainPaper").strip().lower()
    if key in MEDIA_BY_KEY:
        return MEDIA_BY_KEY[key]
    return MEDIA_BY_KEY["plainpaper"]


# Paper Feed Paths (REMOTE1 PP command: m1, m2 parameters)
INPUT_SLOTS: Dict[str, Tuple[str, int, int]] = {
    "Rear":               ("Sheet Feeder (Rear)",                     1, 0),
    "FrontFineArt":       ("Front - Fine Art / Thick Paper",          2, 2),
    "FrontThick":         ("Front - Thick Paper",                     2, 2),
    "FrontPosterBoard":   ("Front - Poster Board (Straight Path)",    2, 0),
    "FrontNoEjectRoller": ("Front - Glossy (No Eject Roller)",        2, 3),
    "DiscTray":           ("CD/DVD Disc Tray",                        2, 1),
    "Roll":               ("Roll Paper",                              3, 0),
}


# Paper sizes: (DisplayName, Width_pt, Height_pt, Width_720dpi, Height_720dpi, SizeID)
@dataclass(frozen=True)
class PaperSizeSpec:
    key: str
    name: str
    width_pt: float
    height_pt: float
    width_720: int
    height_720: int
    size_id: int
    sc_p900_only: bool = False


PAPER_SIZES: List[PaperSizeSpec] = [
    PaperSizeSpec("A4",     "A4 (210 x 297 mm)",              595.28,  841.89,  5952,  8418,   0),
    PaperSizeSpec("A3",     "A3 (297 x 420 mm)",              841.89, 1190.55,  8418, 11906,  62),
    PaperSizeSpec("A3Plus", "A3+ / Super B (329 x 483 mm)",   932.60, 1369.13,  9326, 13692,  61),
    PaperSizeSpec("A2",     "A2 (420 x 594 mm)",             1190.55, 1683.78, 11906, 16838,  67, sc_p900_only=True),
    PaperSizeSpec("A5",     "A5 (148 x 210 mm)",              419.53,  595.28,  4196,  5952,   3),
    PaperSizeSpec("A6",     "A6 (105 x 148 mm)",              297.64,  419.53,  2976,  4196,   4),
    PaperSizeSpec("Letter", "US Letter (8.5 x 11 in)",        612.00,  792.00,  6120,  7920,   1),
    PaperSizeSpec("Legal",  "US Legal (8.5 x 14 in)",         612.00, 1008.00,  6120, 10080,   2),
    PaperSizeSpec("4x6",    "4 x 6 in (102 x 152 mm)",        288.00,  432.00,  2880,  4320,  10),
    PaperSizeSpec("5x7",    "5 x 7 in (127 x 178 mm)",        360.00,  504.00,  3600,  5044,  28),
    PaperSizeSpec("8x10",   "8 x 10 in (203 x 254 mm)",       576.00,  720.00,  5760,  7200,  12),
    PaperSizeSpec("10x12",  "10 x 12 in (254 x 305 mm)",      720.00,  864.00,  7200,  8640,  69),
    PaperSizeSpec("11x14",  "11 x 14 in (279 x 356 mm)",      792.00, 1008.00,  7920, 10080,  65),
    PaperSizeSpec("12x12",  "12 x 12 in (305 x 305 mm)",      864.00,  864.00,  8640,  8640,  70),
    PaperSizeSpec("11x17",  "11 x 17 in / Tabloid",           792.00, 1224.00,  7920, 12240,  64),
    PaperSizeSpec("16x20",  "16 x 20 in (406 x 508 mm)",     1152.00, 1440.00, 11520, 14400,  99, sc_p900_only=True),
    PaperSizeSpec("17x22",  "17 x 22 in / US C (432x559mm)", 1224.00, 1584.00, 12240, 15840,  99, sc_p900_only=True),
]

PAPER_BY_KEY: Dict[str, PaperSizeSpec] = {p.key.lower(): p for p in PAPER_SIZES}


def get_paper_size(page_size: str, custom_w_720: int = 0, custom_h_720: int = 0) -> PaperSizeSpec:
    key = (page_size or "A4").strip().lower()
    if key.startswith("ep"):
        key = key[2:]
    if "." in key:
        key = key.split(".", 1)[0]
    if key in PAPER_BY_KEY:
        return PAPER_BY_KEY[key]
    if custom_w_720 > 0 and custom_h_720 > 0:
        return PaperSizeSpec(
            key="Custom",
            name="Custom Page Size",
            width_pt=custom_w_720 / 10.0,
            height_pt=custom_h_720 / 10.0,
            width_720=custom_w_720,
            height_720=custom_h_720,
            size_id=99,
        )
    return PAPER_BY_KEY["a4"]


# Print Quality Modes (UI options)
QUALITY_MODES: Dict[str, Tuple[str, int]] = {
    "Normal":      ("Standard (720 x 720 dpi)",                     1),
    "Fine":        ("Fine (1440 x 720 dpi)",                        2),
    "High":        ("High Quality Photo (1440 x 1440 dpi)",         3),
    "Max":         ("Max Quality SuperPhoto (5760 x 1440 dpi)",     4),
    "CarbonBlack": ("Max Quality - Carbon Black (5760 x 1440 dpi)", 7),
}


def resolve_firmware_print_quality(media: MediaSpec, quality_key: str, overcoat_val: int) -> int:
    """
    Map user quality choice to the firmware PrintQuality byte in ESC q setq
    supported for the selected media type.
    """
    q = (quality_key or "High").strip().lower()

    # 1. Plain Paper (ppd_id=0, sn_id=0) & Photo Quality Ink Jet (ppd_id=2, sn_id=29):
    #    Only PrintQuality 0 (Standard) and 1 (Quality) exist in firmware.
    if media.ppd_id in (0, 2):
        return 0 if q == "normal" else 1

    # 2. Archival Matte (ppd_id=14, sn_id=15):
    #    Supports PrintQuality 0 (1440x720), 1 (1440x1440), 4 (5760x1440).
    if media.ppd_id == 14:
        if q in ("normal", "fine"):
            return 0
        if q in ("max", "carbonblack"):
            return 4
        return 1

    # 3. Fine Art Matte & Proofing Semimatte (ppd_id in 17, 53, 88, 106, 183):
    #    Supports PrintQuality 1 (1440x1440) and 4 (5760x1440).
    if media.ppd_id in (17, 53, 88, 106, 183):
        return 4 if q in ("max", "carbonblack") else 1

    # 4. Washi, Canvas, CD/DVD (ppd_id in 26, 76, 153, 155, 181):
    #    Supports PrintQuality 1 (1440x1440) only.
    if media.ppd_id in (26, 76, 153, 155, 181):
        return 1

    # 5. Glossy / Semigloss / Luster / Baryta (ppd_id in 13, 15, 27, 145, 157):
    #    Supports 1 (1440x720 BEO Off), 2 (1440x720 BEO On), 3 (1440x1440),
    #    4 (5760x1440), 7 (5760x1440 Carbon Black).
    if q == "carbonblack" and media.supports_carbon_black:
        return 7
    if q in ("max", "carbonblack"):
        return 4
    if q in ("normal", "fine"):
        return 2 if overcoat_val == 2 else 1
    return 3


# ---------------------------------------------------------------------------
# Pure-Python RHV2 Compressor + C Accelerator Loader
# ---------------------------------------------------------------------------

def _build_rhv2_sym_table() -> tuple[list[int], list[int]]:
    code = [0] * 256
    bits = [0] * 256

    code[0], bits[0] = 0x05, 3
    code[1], bits[1] = 0x04, 3
    code[255], bits[255] = 0x00, 3
    code[128], bits[128] = 0x3F, 7

    for k in range(2, 4):
        code[k], bits[k] = (k << 2) | 0x02, 4
        code[256 - k], bits[256 - k] = ((3 - k) << 2) | 0x02, 4
    for k in range(4, 8):
        code[k], bits[k] = (k << 3) | 0x01, 6
        code[256 - k], bits[256 - k] = ((7 - k) << 3) | 0x01, 6
    for k in range(8, 16):
        code[k], bits[k] = (k << 3) | 0x03, 7
        code[256 - k], bits[256 - k] = ((15 - k) << 3) | 0x03, 7
    for k in range(16, 32):
        code[k], bits[k] = (k << 4) | 0x07, 9
        code[256 - k], bits[256 - k] = ((31 - k) << 4) | 0x07, 9
    for k in range(32, 64):
        code[k], bits[k] = (k << 5) | 0x0F, 11
        code[256 - k], bits[256 - k] = ((63 - k) << 5) | 0x0F, 11
    for k in range(64, 128):
        code[k], bits[k] = (k << 6) | 0x1F, 13
        code[256 - k], bits[256 - k] = ((127 - k) << 6) | 0x1F, 13

    return code, bits


_RHV2_SYM_CODE, _RHV2_SYM_BITS = _build_rhv2_sym_table()


def _compress_plane_py(plane: bytes) -> bytes:
    """Compress a single byte plane according to docs/rhv2-spec.md."""
    n = len(plane)
    if n == 0:
        return b""

    out = bytearray()
    acc = 0
    count = 0
    sym_code = _RHV2_SYM_CODE
    sym_bits = _RHV2_SYM_BITS

    def put_bits(val: int, nbits: int) -> None:
        nonlocal acc, count
        acc |= val << count
        count += nbits
        while count >= 8:
            out.append(acc & 0xFF)
            acc >>= 8
            count -= 8

    def emit_rle(
        r: int,
        end_pos: int,
        is_same_run: bool,
        next_last_val: int,
        next_alt_val: int,
    ) -> None:
        nonlocal last_run, repeat_run, last_val, alt_val, pos
        if is_same_run:
            repeat_run = r
        rem = 0 if end_pos == n else r
        while True:
            chunk = rem if rem < 255 else 255
            if chunk == 0:
                s = 128
            elif chunk & 1:
                s = chunk >> 1
            else:
                s = (256 - (chunk >> 1)) & 0xFF
            put_bits(sym_code[s], sym_bits[s])
            if rem <= 254:
                break
            rem -= 255
        last_run = r
        alt_val = next_alt_val
        last_val = next_last_val
        pos = end_pos

    last_run = 1
    repeat_run = 1
    alt_val = 0
    last_val = 255
    mode = 2
    pos = 0

    while pos < n and len(out) <= n:
        v = plane[pos]

        # Case A: Run of last_val
        if v == last_val:
            end_pos = pos + 1
            while end_pos < n and plane[end_pos] == last_val:
                end_pos += 1
            r = end_pos - pos
            if mode == 2:
                put_bits(0x03, 3)
            else:
                put_bits(0x07, 4)
            emit_rle(r, end_pos, r == last_run, last_val, alt_val)
            continue

        # Case B: Run of alt_val with Large Magnitude Step
        if v == alt_val:
            diff = v - last_val
            mag = (diff & 0xFF) if diff >= 0 else (-(v + last_val) & 0xFF)
            if (mag & 0xF8) != 0:
                end_pos = pos + 1
                while end_pos < n and plane[end_pos] == v:
                    end_pos += 1
                r = end_pos - pos
                if mode == 2:
                    put_bits(0x01, 2)
                else:
                    put_bits(0x0F, 5)
                mode = 2
                emit_rle(r, end_pos, r == last_run, v, last_val)
                continue

        # Case C: Final Pixel of Plane OR Immediate Pair/Run of New Value
        if pos + 1 == n or plane[pos + 1] == v:
            delta = v if pos == 0 else ((v - plane[pos - 1]) & 0xFF)
            alt_val = delta
            if pos + 1 == n:
                r = 0
                end_pos = n
            else:
                end_pos = pos + 2
                while end_pos < n and plane[end_pos] == v:
                    end_pos += 1
                r = end_pos - (pos + 1)

            if mode == 4 and r == repeat_run:
                put_bits(0x00, 1)
                r = 0
                last_run = repeat_run
            else:
                if mode == 2:
                    put_bits(0x07, 4)
                else:
                    put_bits(0x01, 2)
            put_bits(sym_code[delta], sym_bits[delta])
            mode = 4

            if r == 0:
                alt_val = last_val
                last_val = v
                pos = end_pos
            else:
                emit_rle(r, end_pos, r == last_run, v, last_val)
            continue

        # Case D: Literal Delta Stream Until End-of-Plane or 3 Identical Pixels
        if mode == 3:
            put_bits(0x00, 1)
        elif mode == 2:
            put_bits(0x0F, 5)
        else:
            put_bits(0x1F, 6)

        k = pos
        while True:
            u = plane[k]
            if u != last_val:
                alt_val = last_val
            delta = u if k == 0 else ((u - plane[k - 1]) & 0xFF)
            put_bits(sym_code[delta], sym_bits[delta])
            last_val = u

            k_next = k + 1
            if k_next == n:
                pos = n
                break
            if k_next + 1 < n and plane[k_next] == u and plane[k_next + 1] == u:
                put_bits(0x7F, 7)
                end_pos = k_next + 2
                while end_pos < n and plane[end_pos] == u:
                    end_pos += 1
                r = end_pos - k_next
                mode = 3
                emit_rle(r, end_pos, r == last_run, u, alt_val)
                break
            k = k_next

    if count > 0:
        out.append(acc & 0xFF)

    if len(out) > n:
        return b"\x7f" + plane
    return bytes(out)




def _load_c_rhv2():
    here = pathlib.Path(__file__).resolve().parent
    so_path = here / "librhv2.so"
    c_path = here / "rhv2.c"
    if not so_path.exists() and c_path.exists():
        try:
            subprocess.run(
                ["gcc", "-O3", "-shared", "-fPIC", str(c_path), "-o", str(so_path)],
                check=True,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
        except Exception:
            pass
    if so_path.exists():
        try:
            lib = ctypes.CDLL(str(so_path))
            lib.rhv2_compress_rgb_band.argtypes = [
                ctypes.c_char_p,
                ctypes.c_char_p,
                ctypes.c_uint32,
                ctypes.c_uint32,
            ]
            lib.rhv2_compress_rgb_band.restype = ctypes.c_uint32
            return lib
        except Exception:
            return None
    return None


_C_RHV2 = _load_c_rhv2()

# Pre-computed RHV2 payload for an all-white band (any width & height)
ALL_WHITE_RHV2_PAYLOAD = bytes.fromhex(
    "00000002000000020000000200000002"
    "fb01000000000000fb01000000000000"
    "fb01000000000000d71f000000000000"
)


def compress_rgb_band_rhv2(rgb_bytes: bytes, print_width: int, height: int) -> bytes:
    """
    Compress a 24-bit RGB band (print_width * height * 3 bytes) into ESC/P-R
    Level 5 RHV2 payload (16-byte plane lengths + 4 8-byte-aligned planes).
    """
    if rgb_bytes.count(0xFF) == len(rgb_bytes):
        return ALL_WHITE_RHV2_PAYLOAD

    aligned_w = (print_width + 7) & ~7
    if _C_RHV2 is not None:
        dst = ctypes.create_string_buffer(aligned_w * height * 4 + 1024)
        out_len = _C_RHV2.rhv2_compress_rgb_band(rgb_bytes, dst, print_width, height)
        if out_len > 0:
            return dst.raw[:out_len]

    # Pure-Python fallback
    pad_px = aligned_w - print_width
    if pad_px > 0:
        row_bytes = print_width * 3
        pad_seq = b"\xff\xff\xff" * pad_px
        padded = bytearray()
        for y in range(height):
            s = y * row_bytes
            padded.extend(rgb_bytes[s : s + row_bytes])
            padded.extend(pad_seq)
        rgb_aligned = bytes(padded)
    else:
        rgb_aligned = rgb_bytes

    r_plane = _compress_plane_py(rgb_aligned[0::3])
    g_plane = _compress_plane_py(rgb_aligned[1::3])
    b_plane = _compress_plane_py(rgb_aligned[2::3])
    x_plane = bytes.fromhex("d71f")
    planes = [r_plane, g_plane, b_plane, x_plane]
    hdr = struct.pack(">IIII", *(len(c) for c in planes))
    body = b"".join(c + b"\x00" * ((8 - (len(c) & 7)) & 7) for c in planes)
    return hdr + body


def make_remote1_cmd(cmd: str, payload: bytes) -> bytes:
    """Build a single 2-char REMOTE1 command with little-endian 16-bit payload length."""
    assert len(cmd) == 2
    return cmd.encode("ascii") + struct.pack("<H", len(payload)) + payload


def make_escpr_packet(cls_char: str, cmd: str, payload: bytes) -> bytes:
    assert len(cls_char) == 1 and len(cmd) == 4
    return (
        b"\x1b"
        + cls_char.encode("ascii")
        + struct.pack("<I", len(payload))
        + cmd.encode("ascii")
        + payload
    )


def make_bsn2_packet(
    rgb_bytes: bytes,
    print_width: int,
    lines: int,
    iy_offset: int,
    ix_offset: int = 0,
) -> bytes:
    """Build a single ESC d <len> bsn2 packet for a band of `lines` rows."""
    hdr = bytearray(52)
    hdr[0] = lines & 0xFF
    struct.pack_into(">H", hdr, 1, print_width)
    struct.pack_into(">I", hdr, 3, ix_offset)
    struct.pack_into(">I", hdr, 7, iy_offset)
    hdr[11] = 1  # CompMode 1 = RHV2 compressed planes
    comp_payload = compress_rgb_band_rhv2(rgb_bytes, print_width, lines)
    return make_escpr_packet("d", "bsn2", bytes(hdr) + comp_payload)


@dataclass
class EscprJobConfig:
    media: str = "PlainPaper"
    input_slot: str = "Rear"
    page_size: str = "A4"
    quality: str = "High"             # Normal, Fine, High, Max, CarbonBlack
    color_mode: str = "EpsonICC"      # EpsonICC, NoColorAdjustment, EpsonStandard, AdvancedBW
    overcoat: str = "Auto"            # Auto, On, Off (Black Enhance Overcoat)
    borderless: bool = False
    unidirectional: bool = False
    margin_720: int = 84              # 84 px @ 720 dpi = 2.96 mm (~3 mm standard margin)
    custom_w_720: int = 0
    custom_h_720: int = 0
    job_name: str = "Linux_ESCPR_Job"
    job_user: str = "linux"
    job_host: str = "localhost"
    user_media_id: int = 0                      # Custom media slot ID (1..30 or 1901..1930; 0 = use MediaSpec)
    print_duty: Optional[int] = None            # Color density adjustment (-50..+50; None = use MediaSpec)
    drying_time: Optional[int] = None           # Drying time per pass (0..255; None = use MediaSpec)
    bottom_edge_priority: Optional[bool] = None # Bottom edge print quality priority (None = use MediaSpec)
    rubbing_reduction: Optional[int] = None     # Rubbing reduction setting (None = use MediaSpec)
    ignore_panel_mismatch: bool = True          # NonCheckPrintMode in ESC u chku (True = print without panel prompt)


def build_remote1_hd_payload(
    cfg: EscprJobConfig,
    media: MediaSpec,
    paper: PaperSizeSpec,
    q_val: int,
    color_byte: int,
    lut_byte: int,
) -> bytes:
    """
    Build the REMOTE1 'HD' TLV metadata packet displayed on the printer's LCD
    and recorded in the printer's job history.
    """
    def tlv_str(tag: int, val_byte: Optional[int], text: str) -> bytes:
        raw = text.encode("utf-8")[:60] + b"\x00"
        if val_byte is None:
            return bytes([tag, len(raw)]) + raw
        return bytes([tag, val_byte & 0xFF, len(raw)]) + raw

    q_label, _ = QUALITY_MODES.get(cfg.quality, ("High Quality (1440 x 1440 dpi)", 3))
    if q_val >= 4:
        res_id, res_str = 5, "SuperPhoto - 5760x1440dpi"
    elif q_val == 3:
        res_id, res_str = 4, "Photo - 1440x1440dpi"
    else:
        res_id, res_str = 3, "Fine - 1440x720dpi"

    color_str = "Black/White" if color_byte == 3 else "Color"
    if lut_byte == 4:
        lut_str = "Off (No Color Adjustment)"
    elif lut_byte == 7:
        lut_str = "Advanced B&W Photo"
    else:
        lut_str = "EPSON Standard (sRGB)"

    hd = bytearray(b"\x00\x05\x01\x02\x07\x04")
    hd += tlv_str(0x01, None, cfg.job_name or "Linux_ESCPR_Job")
    hd += tlv_str(0x02, media.sn_id, media.name)
    hd += tlv_str(0x03, paper.size_id & 0xFF, paper.name)
    hd += tlv_str(0x04, color_byte, color_str)
    hd += tlv_str(0x05, q_val, q_label)
    hd += tlv_str(0x06, 0 if cfg.unidirectional else 1, "Off" if cfg.unidirectional else "On")
    hd += tlv_str(0x07, lut_byte, lut_str)
    hd += tlv_str(0x08, 1, "2.2")
    hd += tlv_str(0x09, 0x19, "0")
    hd += tlv_str(0x0A, 0x19, "0")
    hd += tlv_str(0x0B, 0x19, "0")
    hd += tlv_str(0x17, 0x4B, "0")
    hd += tlv_str(0x18, 0x4B, "0")
    hd += tlv_str(0x1C, res_id, res_str)
    hd += tlv_str(0x1E, 0, "Off")
    return bytes(hd)


def get_slot_margins_720(input_slot: str, borderless: bool, default_margin_720: int = 84) -> Tuple[int, int]:
    """
    Return (margin_left_right_720, margin_top_bottom_720) in 720 dpi pixels
    according to Epson's official SC-P900 PPD ImageableArea table:
    - Borderless: (0, 0)
    - Rear / FrontFineArt / FrontThick / DiscTray: (84, 84) = (3.0 mm, 3.0 mm)
    - FrontPosterBoard / FrontNoEjectRoller:       (84, 566) = (3.0 mm, 20.0 mm)
    """
    if borderless:
        return (0, 0)
    if input_slot in ("FrontPosterBoard", "FrontNoEjectRoller"):
        return (default_margin_720, 566)
    return (default_margin_720, default_margin_720)


def build_job_header(cfg: EscprJobConfig, print_w: int, print_h: int) -> bytes:
    """
    Build the complete EJL + REMOTE1 + ESCPR initialization header.
    """
    media = get_media_spec(cfg.media)
    paper = get_paper_size(cfg.page_size, cfg.custom_w_720, cfg.custom_h_720)
    _, pp_m1, pp_m2 = INPUT_SLOTS.get(cfg.input_slot, INPUT_SLOTS["Rear"])

    # OverCoat (Black Enhance Overcoat): 2 = On, 1 = Off
    if cfg.overcoat.lower() == "on" and not media.is_matte:
        overcoat_val = 2
    elif cfg.overcoat.lower() == "off":
        overcoat_val = 1
    else:
        overcoat_val = media.default_overcoat

    # Effective custom media & mechanical parameters (explicit cfg override -> MediaSpec default)
    eff_user_media_id = resolve_user_media_id(cfg.user_media_id, media)
    raw_duty = cfg.print_duty if cfg.print_duty is not None else media.print_duty
    eff_print_duty = max(-50, min(50, int(raw_duty)))
    raw_drying = cfg.drying_time if cfg.drying_time is not None else media.drying_time
    eff_drying = max(0, min(255, int(raw_drying)))
    eff_bottom_edge = (
        cfg.bottom_edge_priority
        if cfg.bottom_edge_priority is not None
        else media.bottom_edge_priority
    )
    eff_rubbing = (
        cfg.rubbing_reduction
        if cfg.rubbing_reduction is not None
        else media.rubbing_reduction
    )

    # Firmware PrintQuality byte for the selected media type
    q_val = resolve_firmware_print_quality(media, cfg.quality, overcoat_val)

    # Color / LUT mode in ESC q setq and ESC p setq:
    # - EpsonICC or NoColorAdjustment -> Color=0, LUT=4 ('No Color Adjustment' / Host ICC)
    # - EpsonStandard                 -> Color=0, LUT=0 ('EPSON Standard sRGB' firmware LUT)
    # - AdvancedBW                    -> Color=3, LUT=7 ('Advanced B&W Photo')
    cm = cfg.color_mode.lower()
    if cm == "advancedbw":
        color_byte = 3
        lut_byte = 7
    elif cm == "epsonstandard":
        color_byte = 0
        lut_byte = 0
    else:
        color_byte = 0
        lut_byte = 4

    now = datetime.datetime.now()
    ti_payload = struct.pack(
        ">BHBBBBB",
        0,
        now.year,
        now.month,
        now.day,
        now.hour,
        now.minute,
        now.second,
    )

    host_bytes = ("\\\\" + (cfg.job_host or "localhost")).encode("utf-8")[:31]
    user_bytes = (cfg.job_user or "linux").encode("utf-8")[:31]
    hd_payload = build_remote1_hd_payload(cfg, media, paper, q_val, color_byte, lut_byte)

    out = bytearray()
    # 1a. EJL + REMOTE1 Job Initialization
    out += b"\x00\x00\x00\x1b\x01@EJL 1284.4\n@EJL     \n\x1b@\x1b@"
    out += b"\x1b(R\x08\x00\x00REMOTE1"
    out += make_remote1_cmd("TI", ti_payload)
    out += make_remote1_cmd("JS", b"\x00\x00\x00\x00")
    out += make_remote1_cmd("JH", b"\x00\x00\x00\x00\x00\x00" + host_bytes)
    out += make_remote1_cmd("JU", b"\x00\x00\x00" + user_bytes)
    out += make_remote1_cmd("HD", hd_payload)
    out += b"\x1b\x00\x00\x00"

    # 1b. REMOTE1 Page / Print Setting Initialization
    out += b"\x1b(R\x08\x00\x00REMOTE1"
    out += make_remote1_cmd("TI", ti_payload)
    out += make_remote1_cmd("DP", b"\x00\x00")
    out += make_remote1_cmd("SN", b"\x00")
    out += make_remote1_cmd("US", b"\x00\x00\x01")
    out += make_remote1_cmd("US", b"\x00\x01\x00")
    out += make_remote1_cmd("US", b"\x00\x02\x01")
    out += make_remote1_cmd("DR", bytes([0x00, 0x42, 0x00, eff_drying]))
    out += make_remote1_cmd("PP", bytes([0x00, pp_m1, pp_m2]))
    out += b"\x1b\x00\x00\x00"

    # 2. Enter ESC/P-R Level 5 mode
    out += b"\x1b(R\x06\x00\x00ESCPR"

    # 3. ESC q setq (Print quality settings, MUST be 9 bytes: ">BBBBBBBH")
    q_setq = struct.pack(
        ">BBBBBBBH",
        media.sn_id,
        q_val,
        color_byte,
        0, 0, 0,
        3,      # ColorPlane = 3 BPP
        0,      # PaletteSize = 0
    )
    out += make_escpr_packet("q", "setq", q_setq)

    # 4. ESC q seti (Image processing settings, 8 bytes)
    q_seti = bytes([
        1,              # CompressMode = 1 (RHV2 bsn2)
        0,              # StaticAPFSetting
        0,              # ConvertColorSpaceSetting
        0,              # BarcodeMode
        0,              # FaceOrder
        0,              # StandardFastMode
        overcoat_val,   # OverCoatSetting (1=Off, 2=BEO On)
        0,              # GlossPrioritySetting
    ])
    out += make_escpr_packet("q", "seti", q_seti)

    # 5. ESC m setm (Mechanical settings, 7 bytes)
    # Byte 0 = paper.size_id (0x00 = A4, 0x3e = A3, 0x3d = A3+, 0x43 = A2, 0x0a = 4x6, 0x63 = Custom)
    m_setm = bytes([
        paper.size_id & 0xFF,
        1 if cfg.borderless else 0,
        1,                                  # SkipBlankPage = 1
        0x63,                               # DocumentType = 99 (No Duplexer)
        0,                                  # BkPrintMode = 0
        0,                                  # BkInkSaveMode = 0
        1 if cfg.unidirectional else 0,     # CrossDirectionMode
    ])
    out += make_escpr_packet("m", "setm", m_setm)

    # 6. ESC u chku (Print setting check, 2 bytes: [NonCheckPrintMode, CheckPaperSize])
    out += make_escpr_packet("u", "chku", bytes([1 if cfg.ignore_panel_mismatch else 0, 0]))

    # 6b. ESC q setl (Custom Media Type ID, 4 bytes: uint32 BE UserMediaTypeID)
    if eff_user_media_id > 0:
        out += make_escpr_packet("q", "setl", struct.pack(">I", eff_user_media_id))

    # 6c. ESC m seti (Extended mechanical settings, 4 bytes: ">BHB" SingleBkPerm, RubbingReduction, BottomEdgePriority)
    if eff_bottom_edge or eff_rubbing != 0:
        out += make_escpr_packet(
            "m",
            "seti",
            struct.pack(">BHB", 0, eff_rubbing & 0xFFFF, 1 if eff_bottom_edge else 0),
        )

    # 7. ESC j setj (Job / page geometry, 22 bytes, 720 dpi units)
    margin_lr, margin_tb = get_slot_margins_720(cfg.input_slot, cfg.borderless, cfg.margin_720)
    j_setj = struct.pack(
        ">IIhhIIBB",
        paper.width_720,
        paper.height_720,
        margin_tb,
        margin_lr,
        print_w,
        print_h,
        1,  # Input resolution = 1 (720 dpi)
        0,  # Print direction = 0
    )
    out += make_escpr_packet("j", "setj", j_setj)

    # 8. ESC p sttp (Page start, 0 bytes)
    out += make_escpr_packet("p", "sttp", b"")

    # 9. ESC p setq (Page quality / LUT settings, 12 bytes; byte 5 is signed int8 PrintDuty -50..+50)
    p_setq = struct.pack(
        ">BBBHbBBBBBB",
        color_byte,
        3,              # ColorPlane = 3
        lut_byte,       # LUT = 4 (No Color Adjustment / ICC Mode)
        220,            # Gamma = 220 (2.2)
        eff_print_duty, # PrintDuty = -50..+50 (signed int8)
        0, 0, 0, 0, 0, 0,
    )
    out += make_escpr_packet("p", "setq", p_setq)

    return bytes(out)


def normalize_tsnd_jpeg(raw_jpeg: bytes) -> bytes:
    """
    Repack a standard baseline JPEG into the exact segment layout expected by
    the SC-P900 firmware's 'ESC t tsnd' LCD preview decoder (no APP0/JFIF,
    merged DQT, merged DHT in order DC0/DC1/AC0/AC1, DRI, SOF0 and SOS with
    0-based component IDs 0, 1, 2).
    """
    if not raw_jpeg.startswith(b"\xff\xd8"):
        return raw_jpeg

    dqts: List[bytes] = []
    dhts: Dict[int, bytes] = {}
    dri_seg = b"\xff\xdd\x00\x04\x00\x04"
    sof0_payload = b""
    sos_and_scan = b""

    i = 2
    n = len(raw_jpeg)
    while i + 4 <= n and raw_jpeg[i] == 0xFF:
        marker = raw_jpeg[i + 1]
        if marker == 0xDA:  # SOS + entropy scan to EOI
            mlen = struct.unpack_from(">H", raw_jpeg, i + 2)[0]
            sos_hdr = bytearray(raw_jpeg[i + 4 : i + 2 + mlen])
            if len(sos_hdr) >= 6 and sos_hdr[0] == 3:
                sos_hdr[1] = 0
                sos_hdr[3] = 1
                sos_hdr[5] = 2
            sos_and_scan = (
                b"\xff\xda"
                + struct.pack(">H", mlen)
                + bytes(sos_hdr)
                + raw_jpeg[i + 2 + mlen :]
            )
            break
        mlen = struct.unpack_from(">H", raw_jpeg, i + 2)[0]
        payload = raw_jpeg[i + 4 : i + 2 + mlen]
        if marker == 0xDB:
            dqts.append(payload)
        elif marker == 0xC4 and payload:
            dhts[payload[0]] = payload
        elif marker == 0xDD:
            dri_seg = b"\xff\xdd" + struct.pack(">H", mlen) + payload
        elif marker == 0xC0:
            sof = bytearray(payload)
            if len(sof) >= 15 and sof[5] == 3:
                sof[6] = 0
                sof[9] = 1
                sof[12] = 2
            sof0_payload = bytes(sof)
        i += 2 + mlen

    if not (dqts and dhts and sof0_payload and sos_and_scan):
        return raw_jpeg

    dqt_body = b"".join(dqts)
    dht_body = b"".join(dhts[k] for k in (0x00, 0x01, 0x10, 0x11) if k in dhts)
    for k, v in dhts.items():
        if k not in (0x00, 0x01, 0x10, 0x11):
            dht_body += v

    out = bytearray(b"\xff\xd8")
    out += b"\xff\xdb" + struct.pack(">H", len(dqt_body) + 2) + dqt_body
    out += b"\xff\xc4" + struct.pack(">H", len(dht_body) + 2) + dht_body
    out += dri_seg
    out += b"\xff\xc0" + struct.pack(">H", len(sof0_payload) + 2) + sof0_payload
    out += sos_and_scan
    return bytes(out)


def make_tsnd_packet(jpeg_bytes: bytes) -> bytes:
    """Build the ESC t <len> tsnd packet for the printer's LCD preview."""
    norm_jpg = normalize_tsnd_jpeg(jpeg_bytes)
    hdr10 = b"\x00\x01\x00\x00\x00\x00" + struct.pack(">I", len(norm_jpg))
    return make_escpr_packet("t", "tsnd", hdr10 + norm_jpg)


def build_job_footer() -> bytes:
    """Build the ESCPR page/job termination + REMOTE1 LD/JE footer."""
    out = bytearray()
    out += make_escpr_packet("p", "endp", b"\x00")
    out += make_escpr_packet("j", "endj", b"")
    out += (
        b"\x1b@\x1b(R\x08\x00\x00REMOTE1LD\x00\x00\x1b\x00\x00\x00"
        b"\x1b@\x1b(R\x08\x00\x00REMOTE1LD\x00\x00JE\x01\x00\x00\x1b\x00\x00\x00"
    )
    return bytes(out)


def encode_rgb_page_to_escpr(
    rgb_rows: List[bytes],
    print_w: int,
    print_h: int,
    cfg: EscprJobConfig,
    skip_trailing_blank: bool = True,
    thumbnail_jpeg: Optional[bytes] = None,
) -> bytes:
    """
    Encode a 720x720 dpi RGB page into a complete ESC/P-R Level 5 print job.
    `rgb_rows` may contain fewer than `print_h` rows (remaining rows are treated
    as white and skipped at the bottom of the page to save transmission time).
    """
    out = bytearray(build_job_header(cfg, print_w, print_h))
    if thumbnail_jpeg:
        out.extend(make_tsnd_packet(thumbnail_jpeg))

    white_row = b"\xff" * (print_w * 3)

    # Find last non-white row so trailing blank bands at the bottom are skipped
    last_nonblank_y = -1
    for y, row in enumerate(rgb_rows):
        if row != white_row:
            last_nonblank_y = y

    limit_y = ((last_nonblank_y + 16) // 16) * 16 if (skip_trailing_blank and last_nonblank_y >= 0) else print_h
    if limit_y == 0:
        limit_y = 16  # Always send at least 1 band so the page is valid

    band_lines = 16
    for iy in range(0, min(limit_y, print_h), band_lines):
        lines = min(band_lines, print_h - iy)
        # ESC/P-R Level 5 supports 16-line or 8-line bands
        if lines < 16:
            lines = 8 if lines <= 8 else 16
        band_buf = bytearray()
        for dy in range(lines):
            y = iy + dy
            if y < len(rgb_rows):
                band_buf.extend(rgb_rows[y])
            else:
                band_buf.extend(white_row)
        out.extend(make_bsn2_packet(bytes(band_buf), print_w, lines, iy))

    out.extend(build_job_footer())
    return bytes(out)
