//! CUPS PPD and Feature Documentation Generator for Epson SC-P900 / SC-P700.
//!
//! SPDX-License-Identifier: GPL-3.0-or-later

use crate::protocol::{INPUT_SLOTS, PAPER_SIZES, QUALITY_MODES, builtin_media_specs, media_registry};
use std::path::Path;

fn ppd_safe_text(text: &str) -> String {
    let replaced = text
        .replace(['/', ':'], "-")
        .replace('ä', "ae")
        .replace('ö', "oe")
        .replace('ü', "ue")
        .replace('Ä', "Ae")
        .replace('Ö', "Oe")
        .replace('Ü', "Ue")
        .replace('ß', "ss");
    let ascii: String = replaced.chars().filter(char::is_ascii).take(39).collect();
    ascii
}

pub fn build_ppd(_model_name: &str, is_p900: bool) -> String {
    let short_model = if is_p900 { "SC-P900" } else { "SC-P700" };
    let file_name = format!("Epson-{short_model}-ESCPR.ppd");
    let pc_file = &file_name[..file_name.len().min(12)];
    let papers: Vec<_> = PAPER_SIZES
        .iter()
        .filter(|p| is_p900 || !p.sc_p900_only)
        .collect();
    let media_specs = &media_registry().specs;

    let mut lines: Vec<String> = Vec::with_capacity(320);
    lines.push("*PPD-Adobe: \"4.3\"".to_string());
    lines.push("*FormatVersion: \"4.3\"".to_string());
    lines.push("*FileVersion: \"1.0\"".to_string());
    lines.push("*LanguageVersion: English".to_string());
    lines.push("*LanguageEncoding: ISOLatin1".to_string());
    lines.push(format!("*PCFileName: \"{pc_file}\""));
    lines.push("*Manufacturer: \"EPSON\"".to_string());
    lines.push(format!("*Product: \"({short_model} Series)\""));
    lines.push(format!("*ModelName: \"EPSON {short_model} Series\""));
    lines.push(format!("*ShortNickName: \"EPSON {short_model} (ESCPR+ICC)\""));
    lines.push(format!(
        "*NickName: \"EPSON {short_model} Series (SureColor) - Official ICC + ESC/P-R Lv5\""
    ));
    lines.push("*PSVersion: \"(3010.000) 0\"".to_string());
    lines.push("*LanguageLevel: \"3\"".to_string());
    lines.push("*ColorDevice: True".to_string());
    lines.push("*DefaultColorSpace: RGB".to_string());
    lines.push("*FileSystem: False".to_string());
    lines.push("*Throughput: \"1\"".to_string());
    lines.push("*LandscapeOrientation: Plus90".to_string());
    lines.push("*VariablePaperSize: True".to_string());
    lines.push("*TTRasterizer: Type42".to_string());
    lines.push("*cupsVersion: 2.3".to_string());
    lines.push("*cupsModelNumber: 1".to_string());
    lines.push("*cupsManualCopies: False".to_string());
    lines.push(
        "*cupsFilter: \"application/vnd.cups-raster 50 rastertop900escpr\"".to_string(),
    );
    lines.push(format!(
        "*1284DeviceID: \"MFG:EPSON;CMD:ESCPL2,BDC,D4,D4PX,ESCPR5,ESCPR7,END4,GENEP,URF,PWG;MDL:{short_model} Series;CLS:PRINTER;DES:EPSON {short_model} Series;\""
    ));
    lines.push(String::new());

    for m in media_specs {
        let safe_name = ppd_safe_text(&m.name);
        let ppd_key: String = m.key.chars().take(32).collect();
        let icc_val = if m.icc_profile.starts_with('/') {
            m.icc_profile.clone()
        } else {
            format!("/usr/share/color/icc/epson-scp900/{}", m.icc_profile)
        };
        lines.push(format!(
            "*%cupsICCProfile RGB.{ppd_key}./{safe_name}: \"{icc_val}\""
        ));
    }

    lines.push(String::new());
    lines.push("*OpenUI *PageSize/Page Size: PickOne".to_string());
    lines.push("*OrderDependency: 10 AnySetup *PageSize".to_string());
    lines.push("*DefaultPageSize: A4".to_string());
    for p in &papers {
        let sname = p.name.replace('/', "-");
        lines.push(format!(
            "*PageSize {}/{sname}: \"<</PageSize[{:.2} {:.2}]/HWResolution[720 720]/cupsBitsPerColor 8/cupsColorOrder 0/cupsColorSpace 1/ImagingBBox null>>setpagedevice\"",
            p.key, p.width_pt, p.height_pt
        ));
    }
    lines.push("*CloseUI: *PageSize".to_string());
    lines.push(String::new());

    lines.push("*OpenUI *PageRegion/Page Region: PickOne".to_string());
    lines.push("*OrderDependency: 10 AnySetup *PageRegion".to_string());
    lines.push("*DefaultPageRegion: A4".to_string());
    for p in &papers {
        let sname = p.name.replace('/', "-");
        lines.push(format!(
            "*PageRegion {}/{sname}: \"<</PageSize[{:.2} {:.2}]/HWResolution[720 720]/cupsBitsPerColor 8/cupsColorOrder 0/cupsColorSpace 1/ImagingBBox null>>setpagedevice\"",
            p.key, p.width_pt, p.height_pt
        ));
    }
    lines.push("*CloseUI: *PageRegion".to_string());
    lines.push(String::new());

    lines.push("*OpenUI *ColorModel/Color Model: PickOne".to_string());
    lines.push("*OrderDependency: 15 AnySetup *ColorModel".to_string());
    lines.push("*DefaultColorModel: RGB".to_string());
    lines.push("*ColorModel RGB/24-bit RGB Color: \"<</cupsBitsPerColor 8/cupsColorOrder 0/cupsColorSpace 1>>setpagedevice\"".to_string());
    lines.push("*CloseUI: *ColorModel".to_string());
    lines.push(String::new());

    lines.push("*DefaultImageableArea: A4".to_string());
    let m = 8.40_f64;
    for p in &papers {
        let sname = p.name.replace('/', "-");
        lines.push(format!(
            "*ImageableArea {}/{sname}: \"{m:.2} {m:.2} {:.2} {:.2}\"",
            p.key,
            p.width_pt - m,
            p.height_pt - m
        ));
    }
    lines.push(String::new());

    lines.push("*DefaultPaperDimension: A4".to_string());
    for p in &papers {
        let sname = p.name.replace('/', "-");
        lines.push(format!(
            "*PaperDimension {}/{sname}: \"{:.2} {:.2}\"",
            p.key, p.width_pt, p.height_pt
        ));
    }
    lines.push(String::new());

    // MediaType
    lines.push("*OpenUI *MediaType/Media Type: PickOne".to_string());
    lines.push("*OrderDependency: 20 AnySetup *MediaType".to_string());
    lines.push("*DefaultMediaType: PlainPaper".to_string());
    for m in media_specs {
        let safe_name = ppd_safe_text(&m.name);
        let ppd_key: String = m.key.chars().take(32).collect();
        lines.push(format!(
            "*MediaType {ppd_key}/{safe_name}: \"<</MediaType({ppd_key})>>setpagedevice\""
        ));
    }
    lines.push("*CloseUI: *MediaType".to_string());
    lines.push(String::new());

    // InputSlot
    lines.push("*OpenUI *InputSlot/Paper Source: PickOne".to_string());
    lines.push("*OrderDependency: 25 AnySetup *InputSlot".to_string());
    lines.push("*DefaultInputSlot: Rear".to_string());
    for s in INPUT_SLOTS {
        let safe_label = s.name.replace('/', "-");
        lines.push(format!("*InputSlot {}/{safe_label}: \"\"", s.key));
    }
    lines.push("*CloseUI: *InputSlot".to_string());
    lines.push(String::new());

    // PrintQuality
    lines.push("*OpenUI *PrintQuality/Print Quality: PickOne".to_string());
    lines.push("*OrderDependency: 30 AnySetup *PrintQuality".to_string());
    lines.push("*DefaultPrintQuality: High".to_string());
    for (k, label, _) in QUALITY_MODES {
        let safe_label = label.replace('/', "-");
        lines.push(format!("*PrintQuality {k}/{safe_label}: \"\""));
    }
    lines.push("*CloseUI: *PrintQuality".to_string());
    lines.push(String::new());

    // ColorMode
    lines.extend(
        [
            "*OpenUI *EpsonColorMode/Color Management Mode: PickOne",
            "*OrderDependency: 35 AnySetup *EpsonColorMode",
            "*DefaultEpsonColorMode: EpsonICC",
            "*EpsonColorMode EpsonICC/Epson Official ICC Profile (Windows and macOS Quality): \"\"",
            "*EpsonColorMode NoColorAdjustment/No Color Adjustment (Application Managed): \"\"",
            "*EpsonColorMode EpsonStandard/Printer Internal sRGB (Epson Standard LUT): \"\"",
            "*EpsonColorMode AdvancedBW/Advanced Black and White Photo: \"\"",
            "*CloseUI: *EpsonColorMode",
            "",
            "*OpenUI *EpsonRenderingIntent/ICC Rendering Intent: PickOne",
            "*OrderDependency: 40 AnySetup *EpsonRenderingIntent",
            "*DefaultEpsonRenderingIntent: RelativeBPC",
            "*EpsonRenderingIntent RelativeBPC/Relative Colorimetric + Black Point Compensation: \"\"",
            "*EpsonRenderingIntent Perceptual/Perceptual: \"\"",
            "*EpsonRenderingIntent RelativeNoBPC/Relative Colorimetric (No BPC): \"\"",
            "*EpsonRenderingIntent Saturation/Saturation: \"\"",
            "*EpsonRenderingIntent Absolute/Absolute Colorimetric: \"\"",
            "*CloseUI: *EpsonRenderingIntent",
            "",
            "*OpenUI *EpsonOverCoat/Black Enhance Overcoat (BEO): PickOne",
            "*OrderDependency: 45 AnySetup *EpsonOverCoat",
            "*DefaultEpsonOverCoat: Auto",
            "*EpsonOverCoat Auto/Auto (Enabled on Glossy, Luster, Baryta): \"\"",
            "*EpsonOverCoat On/On: \"\"",
            "*EpsonOverCoat Off/Off: \"\"",
            "*CloseUI: *EpsonOverCoat",
            "",
            "*OpenUI *Borderless/Borderless Printing: Boolean",
            "*OrderDependency: 50 AnySetup *Borderless",
            "*DefaultBorderless: False",
            "*Borderless False/Off (Standard 3 mm Margins): \"\"",
            "*Borderless True/On (Borderless): \"\"",
            "*CloseUI: *Borderless",
            "",
            "*OpenUI *Unidirectional/Print Head Direction: Boolean",
            "*OrderDependency: 55 AnySetup *Unidirectional",
            "*DefaultUnidirectional: False",
            "*Unidirectional False/Bidirectional (Faster): \"\"",
            "*Unidirectional True/Unidirectional (Highest Precision): \"\"",
            "*CloseUI: *Unidirectional",
            "",
            "*OpenUI *EpsonPrintDuty/Color Density (Print Duty Override): PickOne",
            "*OrderDependency: 60 AnySetup *EpsonPrintDuty",
            "*DefaultEpsonPrintDuty: Auto",
            "*EpsonPrintDuty Auto/Media Default (0% or Custom Slot Setting): \"\"",
            "*EpsonPrintDuty -30/-30%: \"\"",
            "*EpsonPrintDuty -20/-20%: \"\"",
            "*EpsonPrintDuty -15/-15%: \"\"",
            "*EpsonPrintDuty -10/-10%: \"\"",
            "*EpsonPrintDuty -5/-5%: \"\"",
            "*EpsonPrintDuty 0/0% (Standard): \"\"",
            "*EpsonPrintDuty 5/+5%: \"\"",
            "*EpsonPrintDuty 10/+10%: \"\"",
            "*EpsonPrintDuty 15/+15%: \"\"",
            "*EpsonPrintDuty 20/+20%: \"\"",
            "*EpsonPrintDuty 30/+30%: \"\"",
            "*CloseUI: *EpsonPrintDuty",
            "",
            "*OpenUI *EpsonDryingTime/Ink Drying Time per Pass: PickOne",
            "*OrderDependency: 65 AnySetup *EpsonDryingTime",
            "*DefaultEpsonDryingTime: Auto",
            "*EpsonDryingTime Auto/Media Default: \"\"",
            "*EpsonDryingTime 0/0 (Standard): \"\"",
            "*EpsonDryingTime 1/1: \"\"",
            "*EpsonDryingTime 2/2: \"\"",
            "*EpsonDryingTime 3/3: \"\"",
            "*EpsonDryingTime 5/5: \"\"",
            "*EpsonDryingTime 10/10 (Max): \"\"",
            "*CloseUI: *EpsonDryingTime",
            "",
            "*OpenUI *EpsonBottomEdgePriority/Bottom Edge Print Quality Priority: PickOne",
            "*OrderDependency: 70 AnySetup *EpsonBottomEdgePriority",
            "*DefaultEpsonBottomEdgePriority: Auto",
            "*EpsonBottomEdgePriority Auto/Media Default: \"\"",
            "*EpsonBottomEdgePriority False/Off (Standard Speed): \"\"",
            "*EpsonBottomEdgePriority True/On (Higher Bottom-Edge Precision): \"\"",
            "*CloseUI: *EpsonBottomEdgePriority",
            "",
            "*OpenUI *EpsonIgnorePanelMismatch/Paper Setting Mismatch Check: Boolean",
            "*OrderDependency: 75 AnySetup *EpsonIgnorePanelMismatch",
            "*DefaultEpsonIgnorePanelMismatch: True",
            "*EpsonIgnorePanelMismatch True/Ignore Panel Mismatch (Print Immediately): \"\"",
            "*EpsonIgnorePanelMismatch False/Verify Against Printer Panel Setting: \"\"",
            "*CloseUI: *EpsonIgnorePanelMismatch",
            "",
            "*DefaultFont: Courier",
            "*Font Courier: Standard \"(001.001)\" Standard ROM",
            "",
        ]
        .into_iter()
        .map(String::from),
    );

    lines.join("\n")
}

pub fn generate_ppds(ppd_dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(ppd_dir)?;
    for (model_name, is_p900, fname) in [
        ("SureColor SC-P900", true, "Epson-SC-P900-ESCPR.ppd"),
        ("SureColor SC-P700", false, "Epson-SC-P700-ESCPR.ppd"),
    ] {
        let out = ppd_dir.join(fname);
        std::fs::write(&out, build_ppd(model_name, is_p900))?;
        println!("Generated {}", out.display());
    }
    Ok(())
}

pub fn render_feature_table() -> String {
    let ok = "✅";
    let untested = "🟡";
    let missing = "❌";

    let tested_media = ["PlainPaper", "PhotoPaperGlossy"];
    let tested_slots = ["Rear"];
    let tested_sizes = ["A4"];
    let tested_quality = ["High"];
    let tested_color = ["EpsonICC"];

    let mut l: Vec<String> = Vec::with_capacity(160);
    l.push("<!-- GENERATED by rastertop900escpr --generate-feature-table - do not edit by hand -->".to_string());
    l.push(String::new());
    l.push("# Supported features".to_string());
    l.push(String::new());
    l.push("> [!WARNING]".to_string());
    l.push("> This driver is **beta / experimental**. Only entries marked ✅ have been".to_string());
    l.push("> verified by printing on a real SC-P900. Everything else is implemented from".to_string());
    l.push("> Epson's driver data but has not been printed yet. Reports are welcome.".to_string());
    l.push(String::new());
    l.push(format!(
        "Legend: {ok} tested on hardware · {untested} implemented, untested · {missing} not implemented"
    ));
    l.push(String::new());
    l.push("## Printers".to_string());
    l.push(String::new());
    l.push("| Model | Status |".to_string());
    l.push("|---|---|".to_string());
    l.push(format!("| Epson SureColor SC-P900 | {ok} |"));
    l.push(format!(
        "| Epson SureColor SC-P700 | {untested} same protocol and data family, never printed |"
    ));
    l.push(String::new());
    l.push("## Connections".to_string());
    l.push(String::new());
    l.push("| Status | Connection | URI Example | Notes |".to_string());
    l.push("|---|---|---|---|".to_string());
    l.push(format!(
        "| {ok} | Network (AppSocket / JetDirect) | `socket://<IP>:9100` | Recommended for network printing |"
    ));
    l.push(format!(
        "| {untested} | Network (LPD PASSTHRU) | `lpd://<IP>:515/PASSTHRU` | Discovered automatically in KDE/GNOME |"
    ));
    l.push(format!(
        "| {untested} | USB | `usb://EPSON/SC-P900%20Series?...` | Standard CUPS `usb` backend + IEEE 1284 Device ID; not yet tested on hardware |"
    ));
    l.push(String::new());
    l.push("## Media types".to_string());
    l.push(String::new());
    l.push("Black ink (PK = Photo Black, MK = Matte Black) is chosen by the printer from the media type; the column shows the expected ink, which has only been confirmed where tested.".to_string());
    l.push(String::new());
    l.push("| Status | CUPS `MediaType` | Name | Media ID | Black | ICC profile | Carbon Black |".to_string());
    l.push("|---|---|---|---:|:---:|---|:---:|".to_string());
    for m in builtin_media_specs() {
        let st = if tested_media.contains(&m.key.as_str()) {
            ok
        } else {
            untested
        };
        let blk = if m.is_matte { "MK" } else { "PK" };
        let cb = if m.supports_carbon_black { "yes" } else { "–" };
        l.push(format!(
            "| {st} | `{}` | {} | {} | {blk} | `{}` | {cb} |",
            m.key, m.name, m.sn_id, m.icc_profile
        ));
    }
    l.push(String::new());
    l.push("## Paper sources".to_string());
    l.push(String::new());
    l.push("| Status | CUPS `InputSlot` | Name | Notes |".to_string());
    l.push("|---|---|---|---|".to_string());
    for s in INPUT_SLOTS {
        let (st, note) = if s.key == "Roll" {
            (
                format!("{untested}/{missing}"),
                "feed path selected, but page length, cutting and cut lines are not implemented",
            )
        } else {
            let st_str = if tested_slots.contains(&s.key) {
                ok.to_string()
            } else {
                untested.to_string()
            };
            let note_str = if s.key == "FrontPosterBoard" || s.key == "FrontNoEjectRoller" {
                "3 mm left/right, 20 mm top/bottom margin (per Epson PPD)"
            } else if s.key.starts_with("Front") {
                "3 mm margin on all 4 sides (per Epson PPD)"
            } else {
                ""
            };
            (st_str, note_str)
        };
        l.push(format!("| {st} | `{}` | {} | {note} |", s.key, s.name));
    }
    l.push(String::new());
    l.push("## Paper sizes".to_string());
    l.push(String::new());
    l.push("| Status | CUPS `PageSize` | Name | SC-P900 only |".to_string());
    l.push("|---|---|---|:---:|".to_string());
    for p in PAPER_SIZES {
        let st = if tested_sizes.contains(&p.key) {
            ok
        } else {
            untested
        };
        let p900 = if p.sc_p900_only { "yes" } else { "" };
        l.push(format!("| {st} | `{}` | {} | {p900} |", p.key, p.name));
    }
    l.push(String::new());
    l.push("## Print quality".to_string());
    l.push(String::new());
    l.push("The firmware only accepts certain quality levels per media type; the driver maps the".to_string());
    l.push("requested level to the nearest allowed one (table from Epson's driver data).".to_string());
    l.push(String::new());
    l.push("| Status | CUPS `PrintQuality` | Description |".to_string());
    l.push("|---|---|---|".to_string());
    for (k, desc, _) in QUALITY_MODES {
        let st = if tested_quality.contains(&k) {
            ok
        } else {
            untested
        };
        l.push(format!("| {st} | `{k}` | {desc} |"));
    }
    l.push(String::new());
    l.push("## Color management".to_string());
    l.push(String::new());
    l.push("| Status | CUPS `EpsonColorMode` | Description |".to_string());
    l.push("|---|---|---|".to_string());
    for (k, desc) in [
        (
            "EpsonICC",
            "Convert on the host with the Epson ICC profile for the selected paper; printer color adjustment off",
        ),
        (
            "NoColorAdjustment",
            "Send RGB unchanged, printer color adjustment off (for your own profiles / profiling targets)",
        ),
        (
            "EpsonStandard",
            "Let the printer firmware apply its own sRGB conversion",
        ),
        ("AdvancedBW", "Epson Advanced Black & White Photo mode"),
    ] {
        let st = if tested_color.contains(&k) {
            ok
        } else {
            untested
        };
        l.push(format!("| {st} | `{k}` | {desc} |"));
    }
    l.push(String::new());
    l.push(format!(
        "Source color spaces: the embedded ICC profile of the input image is honoured (sRGB {ok}, Adobe RGB {ok}); images without a profile are treated as sRGB."
    ));
    l.push(String::new());
    l.push(format!(
        "Rendering intents (`EpsonRenderingIntent`): `RelativeBPC` (default, {ok}), `RelativeNoBPC`, `Perceptual`, `Saturation`, `Absolute` ({untested})."
    ));
    l.push(String::new());
    l.push("## Other options".to_string());
    l.push(String::new());
    l.push("| Status | Option | Notes |".to_string());
    l.push("|---|---|---|".to_string());
    l.push(format!(
        "| {ok} | LCD preview thumbnail & job metadata (`ESC t tsnd` + `REMOTE1 HD`) | 272-px baseline JPEG + TLV settings |"
    ));
    l.push(format!(
        "| {untested} | Black Enhance Overcoat (`EpsonOverCoat`) | on by default for glossy media |"
    ));
    l.push(format!(
        "| {untested} | Unidirectional printing (`Unidirectional`) | |"
    ));
    l.push(format!(
        "| {untested} | Custom media (`custom_media.json`, `ESC q setl`, `ESC m seti`, `PrintDuty`, `DryingTime`) | NVRAM slot ID + on-the-fly parameters |"
    ));
    l.push(format!(
        "| {untested}/{missing} | Borderless (`Borderless`) | flag is sent, but the image is not enlarged beyond the edge like Epson's \"Auto Expand\" |"
    ));
    l.push(format!("| {missing} | Duplex, CD/DVD layout helpers | |"));
    l.push(String::new());
    l.join("\n")
}
