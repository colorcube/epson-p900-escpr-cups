#!/usr/bin/env python3
"""
icc_manager.py - LittleCMS2 (PIL.ImageCms) ICC color management pipeline for
                 the Epson SureColor SC-P900 and SC-P700.

Applies the official Epson paper-specific ICC profiles (EE2NE_00..14.ICC /
Epson_SC-P900_700_*.icc) to input RGB images before sending them to the printer
with ESC p setq LUT=4 ("No Color Adjustment" / Host ICC Mode).

SPDX-License-Identifier: GPL-3.0-or-later
"""

from __future__ import annotations

import io
import os
import pathlib
from typing import List, Optional
from PIL import Image, ImageCms

from escpr_protocol import MediaSpec, get_media_spec


def _default_icc_search_dirs() -> List[pathlib.Path]:
    here = pathlib.Path(__file__).resolve().parent
    pkg_icc = here.parent / "icc"
    dirs: List[pathlib.Path] = []
    env_dir = os.environ.get("EPSON_P900_ICC_DIR")
    if env_dir:
        dirs.append(pathlib.Path(env_dir))
    dirs.extend([
        pkg_icc,
        pathlib.Path("/usr/share/color/icc/epson-scp900"),
        pathlib.Path("/usr/local/share/color/icc/epson-scp900"),
        pathlib.Path("/usr/share/color/icc"),
    ])
    return dirs


def find_icc_profile(media_option: str, search_dirs: Optional[List[pathlib.Path]] = None) -> Optional[pathlib.Path]:
    """
    Find the Epson ICC profile path on disk for the requested media type.
    Checks both macOS descriptive names and Windows EE2NE_XX.ICC names.
    """
    media: MediaSpec = get_media_spec(media_option)
    dirs = search_dirs or _default_icc_search_dirs()
    candidates = [
        media.icc_profile,
        media.win_icc,
        media.win_icc.lower(),
    ]
    for d in dirs:
        if not d.is_dir():
            continue
        for name in candidates:
            p = d / name
            if p.is_file():
                return p
    return None


def _resolve_intent_and_flags(rendering_intent: str) -> tuple[int, int]:
    intent_enum = getattr(ImageCms, "Intent", None)
    flags_enum = getattr(ImageCms, "Flags", None)
    flags_dict = getattr(ImageCms, "FLAGS", {})

    bpc_flag = int(
        getattr(flags_enum, "BLACKPOINTCOMPENSATION", flags_dict.get("BLACKPOINTCOMPENSATION", 0x2000))
    )
    highres_flag = int(
        getattr(flags_enum, "HIGHRESPRECALC", flags_dict.get("HIGHRESPRECALC", 0x0400))
    )

    ri = (rendering_intent or "RelativeBPC").strip().lower()
    if ri in ("perceptual", "0"):
        intent = int(getattr(intent_enum, "PERCEPTUAL", 0))
        flags = bpc_flag | highres_flag
    elif ri in ("relative", "relativenobpc", "relativecolorimetricnobpc"):
        intent = int(getattr(intent_enum, "RELATIVE_COLORIMETRIC", 1))
        flags = highres_flag
    elif ri in ("saturation", "2"):
        intent = int(getattr(intent_enum, "SATURATION", 2))
        flags = highres_flag
    elif ri in ("absolute", "absolutecolorimetric", "3"):
        intent = int(getattr(intent_enum, "ABSOLUTE_COLORIMETRIC", 3))
        flags = highres_flag
    else:
        # Default: Relative Colorimetric + Black Point Compensation (Adobe / Windows ICM standard)
        intent = int(getattr(intent_enum, "RELATIVE_COLORIMETRIC", 1))
        flags = bpc_flag | highres_flag

    return intent, flags


def apply_epson_icc_to_image(
    img: Image.Image,
    media_option: str,
    rendering_intent: str = "RelativeBPC",
    source_profile_path: Optional[str] = None,
) -> Image.Image:
    """
    Transform an RGB PIL Image from its embedded ICC profile (or sRGB) into the
    paper-specific Epson SC-P900/SC-P700 ICC profile using LittleCMS2.
    """
    if img.mode != "RGB":
        img = img.convert("RGB")

    dst_icc_path = find_icc_profile(media_option)
    if dst_icc_path is None:
        raise FileNotFoundError(
            f"Could not find Epson ICC profile for media '{media_option}'. "
            f"Run 'icc/fetch_icc_profiles.sh' first."
        )

    # Determine source profile: explicit path -> embedded ICC -> sRGB built-in
    if source_profile_path and pathlib.Path(source_profile_path).is_file():
        src_prof = ImageCms.getOpenProfile(str(source_profile_path))
    elif "icc_profile" in img.info and img.info["icc_profile"]:
        src_prof = ImageCms.ImageCmsProfile(io.BytesIO(img.info["icc_profile"]))
    else:
        src_prof = ImageCms.createProfile("sRGB")

    dst_prof = ImageCms.getOpenProfile(str(dst_icc_path))
    intent, flags = _resolve_intent_and_flags(rendering_intent)

    transform = ImageCms.buildTransform(
        src_prof,
        dst_prof,
        "RGB",
        "RGB",
        renderingIntent=intent,
        flags=flags,
    )
    return ImageCms.applyTransform(img, transform)
