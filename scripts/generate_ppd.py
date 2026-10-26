#!/usr/bin/env python3
"""
generate_ppd.py - Generate CUPS PPD files for Epson SureColor SC-P900 and
                  SC-P700 using the `rastertop900escpr` filter.

SPDX-License-Identifier: GPL-3.0-or-later
"""

from __future__ import annotations

import pathlib
import sys

_HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(_HERE.parent / "src"))

from escpr_protocol import INPUT_SLOTS, MEDIA_SPECS, PAPER_SIZES, QUALITY_MODES


def build_ppd(model_name: str, is_p900: bool) -> str:
    short_model = "SC-P900" if is_p900 else "SC-P700"
    file_name = f"Epson-{short_model}-ESCPR.ppd"
    papers = [p for p in PAPER_SIZES if (is_p900 or not p.sc_p900_only)]

    lines = [
        '*PPD-Adobe: "4.3"',
        '*FormatVersion: "4.3"',
        '*FileVersion: "1.0"',
        '*LanguageVersion: English',
        '*LanguageEncoding: ISOLatin1',
        f'*PCFileName: "{file_name[:12]}"',
        '*Manufacturer: "EPSON"',
        f'*Product: "({model_name})"',
        f'*ModelName: "EPSON {model_name}"',
        f'*ShortNickName: "EPSON {short_model} (ESCPR+ICC)"',
        f'*NickName: "EPSON {model_name} - Official ICC + ESC/P-R Lv5"',
        '*PSVersion: "(3010.000) 0"',
        '*LanguageLevel: "3"',
        '*ColorDevice: True',
        '*DefaultColorSpace: RGB',
        '*FileSystem: False',
        '*Throughput: "1"',
        '*LandscapeOrientation: Plus90',
        '*VariablePaperSize: True',
        '*TTRasterizer: Type42',
        '*cupsVersion: 2.3',
        '*cupsModelNumber: 1',
        '*cupsManualCopies: False',
        '*cupsFilter: "application/vnd.cups-raster 50 rastertop900escpr"',
        f'*1284DeviceID: "MFG:EPSON;CMD:ESCPL2,BDC,D4,D4PX,ESCPR5,END4;MDL:{short_model} Series;CLS:PRINTER;"',
        "",
    ]

    for m in MEDIA_SPECS:
        safe_name = m.name.replace("/", "-")
        lines.append(
            f'*cupsICCProfile RGB.{m.key}./{safe_name}: "/usr/share/color/icc/epson-scp900/{m.icc_profile}"'
        )
    lines.extend([
        "",
        "*OpenUI *PageSize/Page Size: PickOne",
        "*OrderDependency: 10 AnySetup *PageSize",
        "*DefaultPageSize: A4",
    ])

    for p in papers:
        sname = p.name.replace("/", "-")
        lines.append(
            f'*PageSize {p.key}/{sname}: "<</PageSize[{p.width_pt:.2f} {p.height_pt:.2f}]/HWResolution[720 720]/cupsBitsPerColor 8/cupsColorOrder 0/cupsColorSpace 1/ImagingBBox null>>setpagedevice"'
        )
    lines.append("*CloseUI: *PageSize")
    lines.append("")

    lines.extend([
        "*OpenUI *PageRegion/Page Region: PickOne",
        "*OrderDependency: 10 AnySetup *PageRegion",
        "*DefaultPageRegion: A4",
    ])
    for p in papers:
        sname = p.name.replace("/", "-")
        lines.append(
            f'*PageRegion {p.key}/{sname}: "<</PageSize[{p.width_pt:.2f} {p.height_pt:.2f}]/HWResolution[720 720]/cupsBitsPerColor 8/cupsColorOrder 0/cupsColorSpace 1/ImagingBBox null>>setpagedevice"'
        )
    lines.append("*CloseUI: *PageRegion")
    lines.append("")

    lines.extend([
        "*OpenUI *ColorModel/Color Model: PickOne",
        "*OrderDependency: 15 AnySetup *ColorModel",
        "*DefaultColorModel: RGB",
        '*ColorModel RGB/24-bit RGB Color: "<</cupsBitsPerColor 8/cupsColorOrder 0/cupsColorSpace 1>>setpagedevice"',
        "*CloseUI: *ColorModel",
        "",
    ])

    lines.append("*DefaultImageableArea: A4")
    for p in papers:
        sname = p.name.replace("/", "-")
        # 8.4 pt (~2.96 mm) standard margin
        m = 8.40
        lines.append(
            f'*ImageableArea {p.key}/{sname}: "{m:.2f} {m:.2f} {p.width_pt - m:.2f} {p.height_pt - m:.2f}"'
        )
    lines.append("")

    lines.append("*DefaultPaperDimension: A4")
    for p in papers:
        sname = p.name.replace("/", "-")
        lines.append(
            f'*PaperDimension {p.key}/{sname}: "{p.width_pt:.2f} {p.height_pt:.2f}"'
        )
    lines.append("")

    # MediaType
    lines.extend([
        "*OpenUI *MediaType/Media Type: PickOne",
        "*OrderDependency: 20 AnySetup *MediaType",
        "*DefaultMediaType: PlainPaper",
    ])
    for m in MEDIA_SPECS:
        safe_name = m.name.replace("/", "-")
        lines.append(f'*MediaType {m.key}/{safe_name}: "<</MediaType({m.key})>>setpagedevice"')
    lines.append("*CloseUI: *MediaType")
    lines.append("")

    # InputSlot
    lines.extend([
        "*OpenUI *InputSlot/Paper Source: PickOne",
        "*OrderDependency: 25 AnySetup *InputSlot",
        "*DefaultInputSlot: Rear",
    ])
    for k, (label, _m1, _m2) in INPUT_SLOTS.items():
        safe_label = label.replace("/", "-")
        lines.append(f'*InputSlot {k}/{safe_label}: ""')
    lines.append("*CloseUI: *InputSlot")
    lines.append("")

    # PrintQuality
    lines.extend([
        "*OpenUI *PrintQuality/Print Quality: PickOne",
        "*OrderDependency: 30 AnySetup *PrintQuality",
        "*DefaultPrintQuality: High",
    ])
    for k, (label, _qval) in QUALITY_MODES.items():
        safe_label = label.replace("/", "-")
        lines.append(f'*PrintQuality {k}/{safe_label}: ""')
    lines.append("*CloseUI: *PrintQuality")
    lines.append("")

    # ColorMode
    lines.extend([
        "*OpenUI *EpsonColorMode/Color Management Mode: PickOne",
        "*OrderDependency: 35 AnySetup *EpsonColorMode",
        "*DefaultEpsonColorMode: EpsonICC",
        '*EpsonColorMode EpsonICC/Epson Official ICC Profile (Windows and macOS Quality): ""',
        '*EpsonColorMode NoColorAdjustment/No Color Adjustment (Application Managed): ""',
        '*EpsonColorMode EpsonStandard/Printer Internal sRGB (Epson Standard LUT): ""',
        '*EpsonColorMode AdvancedBW/Advanced Black and White Photo: ""',
        "*CloseUI: *EpsonColorMode",
        "",
    ])

    # RenderingIntent
    lines.extend([
        "*OpenUI *EpsonRenderingIntent/ICC Rendering Intent: PickOne",
        "*OrderDependency: 40 AnySetup *EpsonRenderingIntent",
        "*DefaultEpsonRenderingIntent: RelativeBPC",
        '*EpsonRenderingIntent RelativeBPC/Relative Colorimetric + Black Point Compensation: ""',
        '*EpsonRenderingIntent Perceptual/Perceptual: ""',
        '*EpsonRenderingIntent RelativeNoBPC/Relative Colorimetric (No BPC): ""',
        '*EpsonRenderingIntent Saturation/Saturation: ""',
        '*EpsonRenderingIntent Absolute/Absolute Colorimetric: ""',
        "*CloseUI: *EpsonRenderingIntent",
        "",
    ])

    # Black Enhance Overcoat
    lines.extend([
        "*OpenUI *EpsonOverCoat/Black Enhance Overcoat (BEO): PickOne",
        "*OrderDependency: 45 AnySetup *EpsonOverCoat",
        "*DefaultEpsonOverCoat: Auto",
        '*EpsonOverCoat Auto/Auto (Enabled on Glossy, Luster, Baryta): ""',
        '*EpsonOverCoat On/On: ""',
        '*EpsonOverCoat Off/Off: ""',
        "*CloseUI: *EpsonOverCoat",
        "",
    ])

    # Borderless & Unidirectional
    lines.extend([
        "*OpenUI *Borderless/Borderless Printing: Boolean",
        "*OrderDependency: 50 AnySetup *Borderless",
        "*DefaultBorderless: False",
        '*Borderless False/Off (Standard 3 mm Margins): ""',
        '*Borderless True/On (Borderless): ""',
        "*CloseUI: *Borderless",
        "",
        "*OpenUI *Unidirectional/Print Head Direction: Boolean",
        "*OrderDependency: 55 AnySetup *Unidirectional",
        "*DefaultUnidirectional: False",
        '*Unidirectional False/Bidirectional (Faster): ""',
        '*Unidirectional True/Unidirectional (Highest Precision): ""',
        "*CloseUI: *Unidirectional",
        "",
        "*DefaultFont: Courier",
        "*Font Courier: Standard \"(001.001)\" Standard ROM",
        "",
    ])

    return "\n".join(lines)


def main() -> None:
    ppd_dir = _HERE.parent / "ppd"
    ppd_dir.mkdir(parents=True, exist_ok=True)
    for model_name, is_p900, fname in [
        ("SureColor SC-P900", True,  "Epson-SC-P900-ESCPR.ppd"),
        ("SureColor SC-P700", False, "Epson-SC-P700-ESCPR.ppd"),
    ]:
        out = ppd_dir / fname
        out.write_text(build_ppd(model_name, is_p900), encoding="utf-8")
        print(f"Generated {out}")


if __name__ == "__main__":
    main()
