//! ESC/P-R Level 5 & REMOTE1 Protocol Encoder for Epson SureColor SC-P900 and SC-P700.
//!
//! SPDX-License-Identifier: GPL-3.0-or-later

use crate::rhv2::compress_rgb_band;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

#[derive(Debug, Clone)]
pub struct MediaSpec {
    pub key: String,
    pub name: String,
    pub ppd_id: u16,
    pub sn_id: u8,
    pub icc_profile: String,
    pub win_icc: String,
    pub is_matte: bool,
    pub default_overcoat: u8,
    pub supports_carbon_black: bool,
    pub user_media_id: u32,
    pub print_duty: i8,
    pub drying_time: u8,
    pub bottom_edge_priority: bool,
    pub rubbing_reduction: u16,
    #[allow(dead_code)]
    pub slot: u8,
}

impl MediaSpec {
    #[allow(clippy::too_many_arguments)]
    fn builtin(
        key: &str,
        name: &str,
        ppd_id: u16,
        sn_id: u8,
        icc_profile: &str,
        win_icc: &str,
        is_matte: bool,
        default_overcoat: u8,
        supports_carbon_black: bool,
    ) -> Self {
        Self {
            key: key.to_string(),
            name: name.to_string(),
            ppd_id,
            sn_id,
            icc_profile: icc_profile.to_string(),
            win_icc: win_icc.to_string(),
            is_matte,
            default_overcoat,
            supports_carbon_black,
            user_media_id: 0,
            print_duty: 0,
            drying_time: 0,
            bottom_edge_priority: false,
            rubbing_reduction: 0,
            slot: 0,
        }
    }
}

pub fn builtin_media_specs() -> Vec<MediaSpec> {
    vec![
        MediaSpec::builtin(
            "PremiumGlossy",
            "Epson Premium Glossy Photo Paper",
            13,
            11,
            "Epson_SC-P900_700_EpsonPremiumGlossy.icc",
            "EE2NE_04.ICC",
            false,
            1,
            true,
        ),
        MediaSpec::builtin(
            "PremiumSemigloss",
            "Epson Premium Semigloss Photo Paper",
            15,
            12,
            "Epson_SC-P900_700_EpsonPremiumSemigloss.icc",
            "EE2NE_05.ICC",
            false,
            1,
            true,
        ),
        MediaSpec::builtin(
            "PremiumLuster",
            "Epson Premium Luster Photo Paper",
            27,
            13,
            "Epson_SC-P900_700_PremiumLusterPhotoPaper.icc",
            "EE2NE_08.ICC",
            false,
            1,
            true,
        ),
        MediaSpec::builtin(
            "PhotoPaperGlossy",
            "Epson Photo Paper Glossy",
            145,
            52,
            "Epson_SC-P900_700_PhotoPaperGlossy.icc",
            "EE2NE_07.ICC",
            false,
            1,
            true,
        ),
        MediaSpec::builtin(
            "Baryta",
            "Epson Baryta / Traditional Photo Paper",
            157,
            174,
            "Epson_SC-P900_700_Baryta.icc",
            "EE2NE_01.ICC",
            false,
            1,
            true,
        ),
        MediaSpec::builtin(
            "VelvetFineArt",
            "Epson Velvet Fine Art Paper",
            53,
            25,
            "Epson_SC-P900_700_VelvetFineArtPaper.icc",
            "EE2NE_11.ICC",
            true,
            1,
            false,
        ),
        MediaSpec::builtin(
            "UltraSmoothFineArt",
            "Epson UltraSmooth Fine Art Paper",
            88,
            39,
            "Epson_SC-P900_700_UltraSmoothFineArtPaper.icc",
            "EE2NE_10.ICC",
            true,
            1,
            false,
        ),
        MediaSpec::builtin(
            "WatercolorRadiantWhite",
            "Epson Watercolor Paper - Radiant White",
            17,
            16,
            "Epson_SC-P900_700_WatercolorPaperRadiantWhite.icc",
            "EE2NE_13.ICC",
            true,
            1,
            false,
        ),
        MediaSpec::builtin(
            "FineArtMatte",
            "Epson Fine Art Matte (Hot/Cold Press)",
            183,
            206,
            "Epson_SC-P900_700_FineArtMatte.icc",
            "EE2NE_06.ICC",
            true,
            1,
            false,
        ),
        MediaSpec::builtin(
            "Washi",
            "Epson Japanese Kozo Paper Thin (Washi)",
            181,
            205,
            "Epson_SC-P900_700_Washi.icc",
            "EE2NE_12.ICC",
            true,
            1,
            false,
        ),
        MediaSpec::builtin(
            "ArchivalMatte",
            "Epson Archival / Enhanced Matte Paper",
            14,
            15,
            "Epson_SC-P900_700_ArchivalMattePaper.icc",
            "EE2NE_00.ICC",
            true,
            1,
            false,
        ),
        MediaSpec::builtin(
            "PhotoQualityInkJet",
            "Epson Photo Quality Ink Jet Paper",
            2,
            29,
            "Epson_SC-P900_700_ArchivalMattePaper.icc",
            "EE2NE_00.ICC",
            true,
            1,
            false,
        ),
        MediaSpec::builtin(
            "CanvasSatin",
            "Epson Premium Canvas Satin",
            153,
            171,
            "Epson_SC-P900_700_CanvasSatin.icc",
            "EE2NE_03.ICC",
            false,
            1,
            false,
        ),
        MediaSpec::builtin(
            "CanvasMatte",
            "Epson Premium Canvas Matte",
            155,
            169,
            "Epson_SC-P900_700_CanvasMatte.icc",
            "EE2NE_02.ICC",
            true,
            1,
            false,
        ),
        MediaSpec::builtin(
            "PlainPaper",
            "Plain Paper / Bright White Paper",
            0,
            0,
            "Epson_SC-P900_700_Standard.icc",
            "EE2NE_09.ICC",
            true,
            1,
            false,
        ),
        MediaSpec::builtin(
            "ProofingWhiteSemimatte",
            "Epson Proofing Paper White Semimatte",
            106,
            142,
            "Epson_SC-P900_700_ProofingPaperWhiteSemimatte.icc",
            "EE2NE_14.ICC",
            false,
            1,
            false,
        ),
        MediaSpec::builtin(
            "DiscLabel",
            "CD/DVD Disc Label",
            26,
            91,
            "Epson_SC-P900_700_Standard.icc",
            "EE2NE_09.ICC",
            true,
            1,
            false,
        ),
        MediaSpec::builtin(
            "DiscLabelPremium",
            "CD/DVD Premium Surface",
            76,
            92,
            "Epson_SC-P900_700_Standard.icc",
            "EE2NE_09.ICC",
            true,
            1,
            false,
        ),
    ]
}

#[derive(Debug, Deserialize)]
struct CustomMediaJsonEntry {
    #[serde(default)]
    slot: u8,
    #[serde(default)]
    user_media_id: u32,
    #[serde(default)]
    key: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    base_media: String,
    #[serde(default)]
    base_sn_id: Option<u8>,
    #[serde(default)]
    base_ppd_id: Option<u16>,
    #[serde(default)]
    icc_profile: String,
    #[serde(default)]
    win_icc: String,
    #[serde(default)]
    is_matte: Option<bool>,
    #[serde(default)]
    default_overcoat: Option<u8>,
    #[serde(default)]
    supports_carbon_black: Option<bool>,
    #[serde(default)]
    print_duty: i32,
    #[serde(default)]
    drying_time: u16,
    #[serde(default)]
    bottom_edge_priority: bool,
    #[serde(default)]
    rubbing_reduction: u16,
}

fn custom_media_json_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(env_p) = std::env::var("EPSON_P900_CUSTOM_MEDIA_JSON")
        && !env_p.trim().is_empty()
    {
        paths.push(PathBuf::from(env_p));
    }
    paths.push(PathBuf::from("custom_media.json"));
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        paths.push(parent.join("custom_media.json"));
        if let Some(grand) = parent.parent() {
            paths.push(grand.join("custom_media.json"));
            if let Some(root) = grand.parent() {
                paths.push(root.join("custom_media.json"));
            }
        }
    }
    paths.push(PathBuf::from("/etc/epson-p900-escpr-cups/custom_media.json"));
    paths.push(PathBuf::from("/usr/share/epson-p900-escpr-cups/custom_media.json"));
    if let Ok(home) = std::env::var("HOME") {
        let h = PathBuf::from(home);
        paths.push(h.join(".config/epson-p900-escpr/custom_media.json"));
        paths.push(h.join(".config/epson-p900/custom_media.json"));
    }
    paths
}

pub struct MediaRegistry {
    pub specs: Vec<MediaSpec>,
    pub by_key: HashMap<String, usize>,
}

pub fn media_registry() -> &'static MediaRegistry {
    static REGISTRY: OnceLock<MediaRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut specs = builtin_media_specs();
        let mut by_key = HashMap::new();
        for (idx, m) in specs.iter().enumerate() {
            by_key.insert(m.key.to_ascii_lowercase(), idx);
            by_key.insert(m.ppd_id.to_string(), idx);
            by_key.insert(format!("sn_{}", m.sn_id), idx);
        }

        for path in custom_media_json_candidates() {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(entries) = serde_json::from_str::<Vec<CustomMediaJsonEntry>>(&text) else {
                continue;
            };
            for item in entries {
                let slot = item.slot;
                let user_media_id = if item.user_media_id > 0 {
                    item.user_media_id
                } else if (1..=30).contains(&slot) {
                    1900 + u32::from(slot)
                } else {
                    0
                };
                let key = if !item.key.trim().is_empty() {
                    item.key.trim().to_string()
                } else if slot > 0 {
                    format!("CustomSlot{slot}")
                } else {
                    continue;
                };
                if by_key.contains_key(&key.to_ascii_lowercase()) {
                    continue;
                }

                let base_idx = if !item.base_media.is_empty() {
                    by_key
                        .get(&item.base_media.to_ascii_lowercase())
                        .copied()
                        .unwrap_or(5)
                } else if let Some(sn) = item.base_sn_id {
                    by_key.get(&format!("sn_{sn}")).copied().unwrap_or(5)
                } else {
                    5 // VelvetFineArt default
                };
                let base = specs[base_idx].clone();

                let spec = MediaSpec {
                    key: key.clone(),
                    name: if item.name.trim().is_empty() {
                        key.clone()
                    } else {
                        item.name.trim().to_string()
                    },
                    ppd_id: item.base_ppd_id.unwrap_or(base.ppd_id),
                    sn_id: item.base_sn_id.unwrap_or(base.sn_id),
                    icc_profile: if item.icc_profile.trim().is_empty() {
                        base.icc_profile
                    } else {
                        item.icc_profile.trim().to_string()
                    },
                    win_icc: if item.win_icc.trim().is_empty() {
                        base.win_icc
                    } else {
                        item.win_icc.trim().to_string()
                    },
                    is_matte: item.is_matte.unwrap_or(base.is_matte),
                    default_overcoat: item.default_overcoat.unwrap_or(base.default_overcoat),
                    supports_carbon_black: item
                        .supports_carbon_black
                        .unwrap_or(base.supports_carbon_black),
                    user_media_id,
                    print_duty: item.print_duty.clamp(-50, 50) as i8,
                    drying_time: item.drying_time.min(255) as u8,
                    bottom_edge_priority: item.bottom_edge_priority,
                    rubbing_reduction: item.rubbing_reduction,
                    slot,
                };
                let new_idx = specs.len();
                specs.push(spec);
                by_key.insert(key.to_ascii_lowercase(), new_idx);
                if (1..=30).contains(&slot) {
                    by_key.insert(format!("slot_{slot}"), new_idx);
                    by_key.insert(format!("customslot{slot:02}"), new_idx);
                    by_key.insert((1900 + u16::from(slot)).to_string(), new_idx);
                }
                if user_media_id > 0 {
                    by_key.insert(user_media_id.to_string(), new_idx);
                }
            }
        }

        MediaRegistry { specs, by_key }
    })
}

pub fn get_media_spec(media_option: &str) -> MediaSpec {
    let reg = media_registry();
    let key = media_option.trim().to_ascii_lowercase();
    if let Some(&idx) = reg.by_key.get(&key) {
        return reg.specs[idx].clone();
    }
    let plain_idx = reg.by_key.get("plainpaper").copied().unwrap_or(0);
    reg.specs[plain_idx].clone()
}

pub fn resolve_user_media_id(raw_id: u32, media: &MediaSpec) -> u32 {
    let uid = if raw_id > 0 {
        raw_id
    } else {
        media.user_media_id
    };
    let reg = media_registry();
    if (1..=30).contains(&uid) {
        if let Some(&idx) = reg.by_key.get(&format!("slot_{uid}")) {
            let s = &reg.specs[idx];
            if s.user_media_id > 0 {
                return s.user_media_id;
            }
        }
    } else if (1901..=1930).contains(&uid)
        && let Some(&idx) = reg.by_key.get(&format!("slot_{}", uid - 1900))
    {
        let s = &reg.specs[idx];
        if s.user_media_id > 0 {
            return s.user_media_id;
        }
    }
    uid
}

#[derive(Debug, Clone, Copy)]
pub struct InputSlotSpec {
    pub key: &'static str,
    pub name: &'static str,
    pub m1: u8,
    pub m2: u8,
}

pub const INPUT_SLOTS: [InputSlotSpec; 7] = [
    InputSlotSpec {
        key: "Rear",
        name: "Sheet Feeder (Rear)",
        m1: 1,
        m2: 0,
    },
    InputSlotSpec {
        key: "FrontFineArt",
        name: "Front - Fine Art / Thick Paper",
        m1: 2,
        m2: 2,
    },
    InputSlotSpec {
        key: "FrontThick",
        name: "Front - Thick Paper",
        m1: 2,
        m2: 2,
    },
    InputSlotSpec {
        key: "FrontPosterBoard",
        name: "Front - Poster Board (Straight Path)",
        m1: 2,
        m2: 0,
    },
    InputSlotSpec {
        key: "FrontNoEjectRoller",
        name: "Front - Glossy (No Eject Roller)",
        m1: 2,
        m2: 3,
    },
    InputSlotSpec {
        key: "DiscTray",
        name: "CD/DVD Disc Tray",
        m1: 2,
        m2: 1,
    },
    InputSlotSpec {
        key: "Roll",
        name: "Roll Paper",
        m1: 3,
        m2: 0,
    },
];

pub fn get_input_slot(key: &str) -> InputSlotSpec {
    let k = key.trim();
    for s in INPUT_SLOTS {
        if s.key.eq_ignore_ascii_case(k) {
            return s;
        }
    }
    INPUT_SLOTS[0]
}

#[derive(Debug, Clone)]
pub struct PaperSizeSpec {
    pub key: &'static str,
    pub name: &'static str,
    pub width_pt: f64,
    pub height_pt: f64,
    pub width_720: u32,
    pub height_720: u32,
    pub size_id: u8,
    pub sc_p900_only: bool,
}

pub const PAPER_SIZES: [PaperSizeSpec; 17] = [
    PaperSizeSpec {
        key: "A4",
        name: "A4 (210 x 297 mm)",
        width_pt: 595.28,
        height_pt: 841.89,
        width_720: 5952,
        height_720: 8418,
        size_id: 0,
        sc_p900_only: false,
    },
    PaperSizeSpec {
        key: "A3",
        name: "A3 (297 x 420 mm)",
        width_pt: 841.89,
        height_pt: 1190.55,
        width_720: 8418,
        height_720: 11906,
        size_id: 62,
        sc_p900_only: false,
    },
    PaperSizeSpec {
        key: "A3Plus",
        name: "A3+ / Super B (329 x 483 mm)",
        width_pt: 932.60,
        height_pt: 1369.13,
        width_720: 9326,
        height_720: 13692,
        size_id: 61,
        sc_p900_only: false,
    },
    PaperSizeSpec {
        key: "A2",
        name: "A2 (420 x 594 mm)",
        width_pt: 1190.55,
        height_pt: 1683.78,
        width_720: 11906,
        height_720: 16838,
        size_id: 67,
        sc_p900_only: true,
    },
    PaperSizeSpec {
        key: "A5",
        name: "A5 (148 x 210 mm)",
        width_pt: 419.53,
        height_pt: 595.28,
        width_720: 4196,
        height_720: 5952,
        size_id: 3,
        sc_p900_only: false,
    },
    PaperSizeSpec {
        key: "A6",
        name: "A6 (105 x 148 mm)",
        width_pt: 297.64,
        height_pt: 419.53,
        width_720: 2976,
        height_720: 4196,
        size_id: 4,
        sc_p900_only: false,
    },
    PaperSizeSpec {
        key: "Letter",
        name: "US Letter (8.5 x 11 in)",
        width_pt: 612.00,
        height_pt: 792.00,
        width_720: 6120,
        height_720: 7920,
        size_id: 1,
        sc_p900_only: false,
    },
    PaperSizeSpec {
        key: "Legal",
        name: "US Legal (8.5 x 14 in)",
        width_pt: 612.00,
        height_pt: 1008.00,
        width_720: 6120,
        height_720: 10080,
        size_id: 2,
        sc_p900_only: false,
    },
    PaperSizeSpec {
        key: "4x6",
        name: "4 x 6 in (102 x 152 mm)",
        width_pt: 288.00,
        height_pt: 432.00,
        width_720: 2880,
        height_720: 4320,
        size_id: 10,
        sc_p900_only: false,
    },
    PaperSizeSpec {
        key: "5x7",
        name: "5 x 7 in (127 x 178 mm)",
        width_pt: 360.00,
        height_pt: 504.00,
        width_720: 3600,
        height_720: 5044,
        size_id: 28,
        sc_p900_only: false,
    },
    PaperSizeSpec {
        key: "8x10",
        name: "8 x 10 in (203 x 254 mm)",
        width_pt: 576.00,
        height_pt: 720.00,
        width_720: 5760,
        height_720: 7200,
        size_id: 12,
        sc_p900_only: false,
    },
    PaperSizeSpec {
        key: "10x12",
        name: "10 x 12 in (254 x 305 mm)",
        width_pt: 720.00,
        height_pt: 864.00,
        width_720: 7200,
        height_720: 8640,
        size_id: 69,
        sc_p900_only: false,
    },
    PaperSizeSpec {
        key: "11x14",
        name: "11 x 14 in (279 x 356 mm)",
        width_pt: 792.00,
        height_pt: 1008.00,
        width_720: 7920,
        height_720: 10080,
        size_id: 65,
        sc_p900_only: false,
    },
    PaperSizeSpec {
        key: "12x12",
        name: "12 x 12 in (305 x 305 mm)",
        width_pt: 864.00,
        height_pt: 864.00,
        width_720: 8640,
        height_720: 8640,
        size_id: 70,
        sc_p900_only: false,
    },
    PaperSizeSpec {
        key: "11x17",
        name: "11 x 17 in / Tabloid",
        width_pt: 792.00,
        height_pt: 1224.00,
        width_720: 7920,
        height_720: 12240,
        size_id: 64,
        sc_p900_only: false,
    },
    PaperSizeSpec {
        key: "16x20",
        name: "16 x 20 in (406 x 508 mm)",
        width_pt: 1152.00,
        height_pt: 1440.00,
        width_720: 11520,
        height_720: 14400,
        size_id: 99,
        sc_p900_only: true,
    },
    PaperSizeSpec {
        key: "17x22",
        name: "17 x 22 in / US C (432x559mm)",
        width_pt: 1224.00,
        height_pt: 1584.00,
        width_720: 12240,
        height_720: 15840,
        size_id: 99,
        sc_p900_only: true,
    },
];

pub fn get_paper_size(page_size: &str, custom_w_720: u32, custom_h_720: u32) -> PaperSizeSpec {
    let mut k = page_size.trim();
    if k.len() > 2 && k[..2].eq_ignore_ascii_case("ep") {
        k = &k[2..];
    }
    if let Some((base, _)) = k.split_once('.') {
        k = base;
    }
    for p in PAPER_SIZES {
        if p.key.eq_ignore_ascii_case(k) {
            return p;
        }
    }
    if custom_w_720 > 0 && custom_h_720 > 0 {
        return PaperSizeSpec {
            key: "Custom",
            name: "Custom Page Size",
            width_pt: f64::from(custom_w_720) / 10.0,
            height_pt: f64::from(custom_h_720) / 10.0,
            width_720: custom_w_720,
            height_720: custom_h_720,
            size_id: 99,
            sc_p900_only: false,
        };
    }
    PAPER_SIZES[0].clone()
}

pub const QUALITY_MODES: [(&str, &str, u8); 5] = [
    ("Normal", "Standard (720 x 720 dpi)", 1),
    ("Fine", "Fine (1440 x 720 dpi)", 2),
    ("High", "High Quality Photo (1440 x 1440 dpi)", 3),
    ("Max", "Max Quality SuperPhoto (5760 x 1440 dpi)", 4),
    (
        "CarbonBlack",
        "Max Quality - Carbon Black (5760 x 1440 dpi)",
        7,
    ),
];

pub fn resolve_firmware_print_quality(media: &MediaSpec, quality_key: &str, overcoat_val: u8) -> u8 {
    let q = quality_key.trim().to_ascii_lowercase();
    if media.ppd_id == 0 || media.ppd_id == 2 {
        return if q == "normal" { 0 } else { 1 };
    }
    if media.ppd_id == 14 {
        if q == "normal" || q == "fine" {
            return 0;
        }
        if q == "max" || q == "carbonblack" {
            return 4;
        }
        return 1;
    }
    if matches!(media.ppd_id, 17 | 53 | 88 | 106 | 183) {
        return if q == "max" || q == "carbonblack" {
            4
        } else {
            1
        };
    }
    if matches!(media.ppd_id, 26 | 76 | 153 | 155 | 181) {
        return 1;
    }
    if q == "carbonblack" && media.supports_carbon_black {
        return 7;
    }
    if q == "max" || q == "carbonblack" {
        return 4;
    }
    if q == "normal" || q == "fine" {
        return if overcoat_val == 2 { 2 } else { 1 };
    }
    3
}

pub fn get_slot_margins_720(input_slot: &str, borderless: bool, default_margin_720: i16) -> (i16, i16) {
    if borderless {
        return (0, 0);
    }
    if input_slot.eq_ignore_ascii_case("FrontPosterBoard")
        || input_slot.eq_ignore_ascii_case("FrontNoEjectRoller")
    {
        return (default_margin_720, 566);
    }
    (default_margin_720, default_margin_720)
}

#[derive(Debug, Clone)]
pub struct EscprJobConfig {
    pub media: String,
    pub input_slot: String,
    pub page_size: String,
    pub quality: String,
    pub color_mode: String,
    pub overcoat: String,
    pub borderless: bool,
    pub unidirectional: bool,
    pub margin_720: i16,
    pub custom_w_720: u32,
    pub custom_h_720: u32,
    pub job_name: String,
    pub job_user: String,
    pub job_host: String,
    pub user_media_id: u32,
    pub print_duty: Option<i8>,
    pub drying_time: Option<u8>,
    pub bottom_edge_priority: Option<bool>,
    pub rubbing_reduction: Option<u16>,
    pub ignore_panel_mismatch: bool,
}

impl Default for EscprJobConfig {
    fn default() -> Self {
        Self {
            media: "PlainPaper".to_string(),
            input_slot: "Rear".to_string(),
            page_size: "A4".to_string(),
            quality: "High".to_string(),
            color_mode: "EpsonICC".to_string(),
            overcoat: "Auto".to_string(),
            borderless: false,
            unidirectional: false,
            margin_720: 84,
            custom_w_720: 0,
            custom_h_720: 0,
            job_name: "Linux_ESCPR_Job".to_string(),
            job_user: "linux".to_string(),
            job_host: "localhost".to_string(),
            user_media_id: 0,
            print_duty: None,
            drying_time: None,
            bottom_edge_priority: None,
            rubbing_reduction: None,
            ignore_panel_mismatch: true,
        }
    }
}

pub fn make_remote1_cmd(cmd: &[u8; 2], payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + payload.len());
    out.extend_from_slice(cmd);
    out.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    out.extend_from_slice(payload);
    out
}

pub fn make_escpr_packet(cls: u8, cmd: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(10 + payload.len());
    out.push(0x1B);
    out.push(cls);
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(cmd);
    out.extend_from_slice(payload);
    out
}

pub fn make_bsn2_packet(
    rgb_bytes: &[u8],
    print_width: usize,
    lines: usize,
    iy_offset: u32,
    ix_offset: u32,
) -> Vec<u8> {
    let mut hdr = [0u8; 52];
    hdr[0] = (lines & 0xFF) as u8;
    hdr[1..3].copy_from_slice(&(print_width as u16).to_be_bytes());
    hdr[3..7].copy_from_slice(&ix_offset.to_be_bytes());
    hdr[7..11].copy_from_slice(&iy_offset.to_be_bytes());
    hdr[11] = 1; // CompMode 1 = RHV2 compressed planes
    let comp = compress_rgb_band(rgb_bytes, print_width, lines);
    let mut payload = Vec::with_capacity(52 + comp.len());
    payload.extend_from_slice(&hdr);
    payload.extend_from_slice(&comp);
    make_escpr_packet(b'd', b"bsn2", &payload)
}

fn local_timestamp_payload() -> [u8; 8] {
    let mut ti = [0u8; 8];
    unsafe {
        let mut now: libc::time_t = 0;
        libc::time(&mut now);
        let mut tm_val: libc::tm = std::mem::zeroed();
        if !libc::localtime_r(&now, &mut tm_val).is_null() {
            let year = (tm_val.tm_year + 1900) as u16;
            ti[0] = 0;
            ti[1..3].copy_from_slice(&year.to_be_bytes());
            ti[3] = (tm_val.tm_mon + 1) as u8;
            ti[4] = tm_val.tm_mday as u8;
            ti[5] = tm_val.tm_hour as u8;
            ti[6] = tm_val.tm_min as u8;
            ti[7] = tm_val.tm_sec as u8;
            return ti;
        }
    }
    [0, 0x07, 0xEA, 10, 26, 12, 0, 0]
}

fn build_remote1_hd_payload(
    cfg: &EscprJobConfig,
    media: &MediaSpec,
    paper: &PaperSizeSpec,
    q_val: u8,
    overcoat_val: u8,
    color_byte: u8,
    lut_byte: u8,
) -> Vec<u8> {
    fn tlv_str(tag: u8, val_byte: Option<u8>, text: &str) -> Vec<u8> {
        let mut raw: Vec<u8> = text.as_bytes().iter().copied().take(60).collect();
        raw.push(0);
        let mut v = Vec::with_capacity(3 + raw.len());
        v.push(tag);
        if let Some(vb) = val_byte {
            v.push(vb);
        }
        v.push(raw.len() as u8);
        v.extend_from_slice(&raw);
        v
    }

    let q_label = QUALITY_MODES
        .iter()
        .find(|(k, _, _)| k.eq_ignore_ascii_case(&cfg.quality))
        .map(|(_, l, _)| *l)
        .unwrap_or("High Quality (1440 x 1440 dpi)");

    let (res_id, res_str) = if q_val >= 4 {
        (5u8, "SuperPhoto - 5760x1440dpi")
    } else if q_val == 3 {
        (4u8, "Photo - 1440x1440dpi")
    } else {
        (3u8, "Fine - 1440x720dpi")
    };

    let color_str = if color_byte == 3 {
        "Black/White"
    } else {
        "Color"
    };
    let lut_str = if lut_byte == 4 {
        "Off (No Color Adjustment)"
    } else if lut_byte == 6 {
        "Adobe RGB"
    } else if lut_byte == 7 {
        "Advanced B&W Photo"
    } else {
        "EPSON Standard (sRGB)"
    };

    let overcoat_on = overcoat_val == 2;

    let mut hd = vec![0x00, 0x05, 0x01, 0x02, 0x07, 0x04];
    let job_title = if cfg.job_name.is_empty() {
        "Linux_ESCPR_Job"
    } else {
        &cfg.job_name
    };
    hd.extend_from_slice(&tlv_str(0x01, None, job_title));
    hd.extend_from_slice(&tlv_str(0x02, Some(media.sn_id), &media.name));
    hd.extend_from_slice(&tlv_str(0x03, Some(paper.size_id), paper.name));
    hd.extend_from_slice(&tlv_str(0x04, Some(color_byte), color_str));
    hd.extend_from_slice(&tlv_str(0x05, Some(q_val), q_label));
    hd.extend_from_slice(&tlv_str(
        0x06,
        Some(if overcoat_on { 1 } else { 0 }),
        if overcoat_on { "On" } else { "Off" },
    ));
    hd.extend_from_slice(&tlv_str(0x07, Some(lut_byte), lut_str));
    hd.extend_from_slice(&tlv_str(0x08, Some(1), "2.2"));
    hd.extend_from_slice(&tlv_str(0x09, Some(0x19), "0"));
    hd.extend_from_slice(&tlv_str(0x0A, Some(0x19), "0"));
    hd.extend_from_slice(&tlv_str(0x0B, Some(0x19), "0"));
    hd.extend_from_slice(&tlv_str(0x17, Some(0x4B), "0"));
    hd.extend_from_slice(&tlv_str(0x18, Some(0x4B), "0"));
    hd.extend_from_slice(&tlv_str(0x1C, Some(res_id), res_str));
    hd.extend_from_slice(&tlv_str(0x1E, Some(0), "Off"));
    hd
}

pub fn build_job_header(cfg: &EscprJobConfig, print_w: u32, print_h: u32) -> Vec<u8> {
    let media = get_media_spec(&cfg.media);
    let paper = get_paper_size(&cfg.page_size, cfg.custom_w_720, cfg.custom_h_720);
    let slot = get_input_slot(&cfg.input_slot);

    let overcoat_val = if cfg.overcoat.eq_ignore_ascii_case("on") && !media.is_matte {
        2u8
    } else if cfg.overcoat.eq_ignore_ascii_case("off") {
        1u8
    } else {
        media.default_overcoat
    };

    let eff_user_media_id = resolve_user_media_id(cfg.user_media_id, &media);
    let eff_print_duty = cfg.print_duty.unwrap_or(media.print_duty).clamp(-50, 50);
    let eff_drying = cfg.drying_time.unwrap_or(media.drying_time);
    let eff_bottom_edge = cfg
        .bottom_edge_priority
        .unwrap_or(media.bottom_edge_priority);
    let eff_rubbing = cfg.rubbing_reduction.unwrap_or(media.rubbing_reduction);

    let q_val = resolve_firmware_print_quality(&media, &cfg.quality, overcoat_val);

    let (color_byte, lut_byte) = if cfg.color_mode.eq_ignore_ascii_case("advancedbw") {
        (3u8, 7u8)
    } else if cfg.color_mode.eq_ignore_ascii_case("epsonadobergb")
        || cfg.color_mode.eq_ignore_ascii_case("adobergb")
    {
        (0u8, 6u8)
    } else if cfg.color_mode.eq_ignore_ascii_case("epsonstandard")
        || cfg.color_mode.eq_ignore_ascii_case("srgb")
    {
        (0u8, 0u8)
    } else {
        (0u8, 4u8)
    };

    let ti_payload = local_timestamp_payload();
    let host_str = format!(
        "\\\\{}",
        if cfg.job_host.is_empty() {
            "localhost"
        } else {
            &cfg.job_host
        }
    );
    let host_bytes: Vec<u8> = host_str.as_bytes().iter().copied().take(31).collect();
    let user_bytes: Vec<u8> = (if cfg.job_user.is_empty() {
        "linux"
    } else {
        &cfg.job_user
    })
    .as_bytes()
    .iter()
    .copied()
    .take(31)
    .collect();

    let hd_payload = build_remote1_hd_payload(
        cfg,
        &media,
        &paper,
        q_val,
        overcoat_val,
        color_byte,
        lut_byte,
    );

    let mut out = Vec::with_capacity(1024);
    // 1a. EJL + REMOTE1 Job Initialization
    out.extend_from_slice(b"\x00\x00\x00\x1b\x01@EJL 1284.4\n@EJL     \n\x1b@\x1b@");
    out.extend_from_slice(b"\x1b(R\x08\x00\x00REMOTE1");
    out.extend_from_slice(&make_remote1_cmd(b"TI", &ti_payload));
    out.extend_from_slice(&make_remote1_cmd(b"JS", &[0, 0, 0, 0]));
    let mut jh = vec![0u8; 6];
    jh.extend_from_slice(&host_bytes);
    out.extend_from_slice(&make_remote1_cmd(b"JH", &jh));
    let mut ju = vec![0u8; 3];
    ju.extend_from_slice(&user_bytes);
    out.extend_from_slice(&make_remote1_cmd(b"JU", &ju));
    out.extend_from_slice(&make_remote1_cmd(b"HD", &hd_payload));
    out.extend_from_slice(b"\x1b\x00\x00\x00");

    // 1b. REMOTE1 Page / Print Setting Initialization
    out.extend_from_slice(b"\x1b(R\x08\x00\x00REMOTE1");
    out.extend_from_slice(&make_remote1_cmd(b"TI", &ti_payload));
    out.extend_from_slice(&make_remote1_cmd(b"DP", &[0, 0]));
    out.extend_from_slice(&make_remote1_cmd(b"SN", &[0]));
    out.extend_from_slice(&make_remote1_cmd(b"US", &[0, 0, 1]));
    out.extend_from_slice(&make_remote1_cmd(b"US", &[0, 1, 0]));
    out.extend_from_slice(&make_remote1_cmd(b"US", &[0, 2, 1]));
    out.extend_from_slice(&make_remote1_cmd(b"DR", &[0x00, 0x42, 0x00, eff_drying]));
    out.extend_from_slice(&make_remote1_cmd(b"PP", &[0x00, slot.m1, slot.m2]));
    out.extend_from_slice(b"\x1b\x00\x00\x00");

    // 2. Enter ESC/P-R Level 5 mode
    out.extend_from_slice(b"\x1b(R\x06\x00\x00ESCPR");

    // 3. ESC q setq (9 bytes)
    let q_setq = [media.sn_id, q_val, color_byte, 0, 0, 0, 3, 0, 0];
    out.extend_from_slice(&make_escpr_packet(b'q', b"setq", &q_setq));

    // 4. ESC q seti (8 bytes)
    let q_seti = [1, 0, 0, 0, 0, 0, overcoat_val, 0];
    out.extend_from_slice(&make_escpr_packet(b'q', b"seti", &q_seti));

    // 5. ESC m setm (7 bytes)
    let m_setm = [
        paper.size_id,
        u8::from(cfg.borderless),
        1,
        0x63,
        0,
        0,
        u8::from(cfg.unidirectional),
    ];
    out.extend_from_slice(&make_escpr_packet(b'm', b"setm", &m_setm));

    // 6. ESC u chku (2 bytes)
    out.extend_from_slice(&make_escpr_packet(
        b'u',
        b"chku",
        &[u8::from(cfg.ignore_panel_mismatch), 0],
    ));

    // 6b. ESC q setl (4 bytes BE UserMediaTypeID)
    if eff_user_media_id > 0 {
        out.extend_from_slice(&make_escpr_packet(
            b'q',
            b"setl",
            &eff_user_media_id.to_be_bytes(),
        ));
    }

    // 6c. ESC m seti (4 bytes)
    if eff_bottom_edge || eff_rubbing != 0 {
        let mut m_seti = [0u8; 4];
        m_seti[0] = 0;
        m_seti[1..3].copy_from_slice(&eff_rubbing.to_be_bytes());
        m_seti[3] = u8::from(eff_bottom_edge);
        out.extend_from_slice(&make_escpr_packet(b'm', b"seti", &m_seti));
    }

    // 7. ESC j setj (22 bytes)
    let (margin_lr, margin_tb) = get_slot_margins_720(&cfg.input_slot, cfg.borderless, cfg.margin_720);
    let mut j_setj = [0u8; 22];
    j_setj[0..4].copy_from_slice(&paper.width_720.to_be_bytes());
    j_setj[4..8].copy_from_slice(&paper.height_720.to_be_bytes());
    j_setj[8..10].copy_from_slice(&margin_tb.to_be_bytes());
    j_setj[10..12].copy_from_slice(&margin_lr.to_be_bytes());
    j_setj[12..16].copy_from_slice(&print_w.to_be_bytes());
    j_setj[16..20].copy_from_slice(&print_h.to_be_bytes());
    j_setj[20] = 1; // 720 dpi
    j_setj[21] = 0;
    out.extend_from_slice(&make_escpr_packet(b'j', b"setj", &j_setj));

    // 8. ESC p sttp (0 bytes)
    out.extend_from_slice(&make_escpr_packet(b'p', b"sttp", &[]));

    // 9. ESC p setq (12 bytes)
    let mut p_setq = [0u8; 12];
    p_setq[0] = color_byte;
    p_setq[1] = 3;
    p_setq[2] = lut_byte;
    p_setq[3..5].copy_from_slice(&220u16.to_be_bytes());
    p_setq[5] = eff_print_duty as u8;
    out.extend_from_slice(&make_escpr_packet(b'p', b"setq", &p_setq));

    out
}

pub fn build_job_footer() -> Vec<u8> {
    let mut out = Vec::with_capacity(128);
    out.extend_from_slice(&make_escpr_packet(b'p', b"endp", &[0x00]));
    out.extend_from_slice(&make_escpr_packet(b'j', b"endj", &[]));
    out.extend_from_slice(
        b"\x1b@\x1b(R\x08\x00\x00REMOTE1LD\x00\x00\x1b\x00\x00\x00\
          \x1b@\x1b(R\x08\x00\x00REMOTE1LD\x00\x00JE\x01\x00\x00\x1b\x00\x00\x00",
    );
    out
}

/// Repack a baseline JPEG into the exact segment layout expected by the SC-P900
/// firmware's `ESC t tsnd` LCD preview decoder.
pub fn normalize_tsnd_jpeg(raw_jpeg: &[u8]) -> Vec<u8> {
    if !raw_jpeg.starts_with(&[0xFF, 0xD8]) {
        return raw_jpeg.to_vec();
    }

    let mut dqts: Vec<Vec<u8>> = Vec::new();
    let mut dhts: HashMap<u8, Vec<u8>> = HashMap::new();
    let mut dri_seg: Vec<u8> = Vec::new();
    let mut sof0_payload: Vec<u8> = Vec::new();
    let mut sos_and_scan: Vec<u8> = Vec::new();

    let mut i = 2usize;
    let n = raw_jpeg.len();
    while i + 4 <= n && raw_jpeg[i] == 0xFF {
        let marker = raw_jpeg[i + 1];
        let mlen = u16::from_be_bytes([raw_jpeg[i + 2], raw_jpeg[i + 3]]) as usize;
        if i + 2 + mlen > n {
            break;
        }
        if marker == 0xDA {
            let mut sos_hdr = raw_jpeg[i + 4..i + 2 + mlen].to_vec();
            if sos_hdr.len() >= 6 && sos_hdr[0] == 3 {
                sos_hdr[1] = 0;
                sos_hdr[3] = 1;
                sos_hdr[5] = 2;
            }
            sos_and_scan.extend_from_slice(&[0xFF, 0xDA]);
            sos_and_scan.extend_from_slice(&(mlen as u16).to_be_bytes());
            sos_and_scan.extend_from_slice(&sos_hdr);
            sos_and_scan.extend_from_slice(&raw_jpeg[i + 2 + mlen..]);
            break;
        }
        let payload = &raw_jpeg[i + 4..i + 2 + mlen];
        match marker {
            0xDB => dqts.push(payload.to_vec()),
            0xC4 if !payload.is_empty() => {
                dhts.insert(payload[0], payload.to_vec());
            }
            0xDD => {
                dri_seg.clear();
                dri_seg.extend_from_slice(&[0xFF, 0xDD]);
                dri_seg.extend_from_slice(&(mlen as u16).to_be_bytes());
                dri_seg.extend_from_slice(payload);
            }
            0xC0 => {
                let mut sof = payload.to_vec();
                if sof.len() >= 15 && sof[5] == 3 {
                    sof[6] = 0;
                    sof[9] = 1;
                    sof[12] = 2;
                }
                sof0_payload = sof;
            }
            _ => {}
        }
        i += 2 + mlen;
    }

    if dqts.is_empty() || dhts.is_empty() || sof0_payload.is_empty() || sos_and_scan.is_empty() {
        return raw_jpeg.to_vec();
    }

    let mut dqt_body = Vec::new();
    for d in &dqts {
        dqt_body.extend_from_slice(d);
    }
    let mut dht_body = Vec::new();
    for k in [0x00u8, 0x01, 0x10, 0x11] {
        if let Some(v) = dhts.get(&k) {
            dht_body.extend_from_slice(v);
        }
    }
    for (k, v) in &dhts {
        if !matches!(k, 0x00 | 0x01 | 0x10 | 0x11) {
            dht_body.extend_from_slice(v);
        }
    }

    let mut out = Vec::with_capacity(raw_jpeg.len() + 16);
    out.extend_from_slice(&[0xFF, 0xD8]);
    out.extend_from_slice(&[0xFF, 0xDB]);
    out.extend_from_slice(&((dqt_body.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(&dqt_body);
    out.extend_from_slice(&[0xFF, 0xC4]);
    out.extend_from_slice(&((dht_body.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(&dht_body);
    out.extend_from_slice(&dri_seg);
    out.extend_from_slice(&[0xFF, 0xC0]);
    out.extend_from_slice(&((sof0_payload.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(&sof0_payload);
    out.extend_from_slice(&sos_and_scan);
    out
}

pub fn make_tsnd_packet(raw_jpeg: &[u8]) -> Vec<u8> {
    let norm = normalize_tsnd_jpeg(raw_jpeg);
    let mut payload = Vec::with_capacity(10 + norm.len());
    payload.extend_from_slice(&[0x00, 0x01, 0x00, 0x00, 0x00, 0x00]);
    payload.extend_from_slice(&(norm.len() as u32).to_be_bytes());
    payload.extend_from_slice(&norm);
    make_escpr_packet(b't', b"tsnd", &payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_custom_media_resolution_and_job_header() {
        let m = get_media_spec("Slot01_Example_Luster_250");
        assert_eq!(m.slot, 1);
        assert_eq!(m.sn_id, 13); // PremiumLuster
        assert_eq!(m.user_media_id, 915_907_607);
        assert_eq!(resolve_user_media_id(1, &m), 915_907_607);
        assert_eq!(resolve_user_media_id(1901, &m), 915_907_607);

        let cfg = EscprJobConfig {
            media: "Slot01_Example_Luster_250".to_string(),
            print_duty: Some(-10),
            bottom_edge_priority: Some(true),
            ..Default::default()
        };
        let hdr = build_job_header(&cfg, 5784, 8250);
        // Verify ESC q setl packet: 1b 'q' 04 00 00 00 "setl" 36 97 a4 17
        let expected_setl = b"\x1bq\x04\x00\x00\x00setl\x36\x97\xa4\x17";
        assert!(
            hdr.windows(expected_setl.len())
                .any(|w| w == expected_setl),
            "ESC q setl with MA_ID 0x3697a417 not found in job header"
        );
        // Verify ESC m seti packet: 1b 'm' 04 00 00 00 "seti" 00 00 00 01
        let expected_seti = b"\x1bm\x04\x00\x00\x00seti\x00\x00\x00\x01";
        assert!(
            hdr.windows(expected_seti.len())
                .any(|w| w == expected_seti),
            "ESC m seti not found in job header"
        );
    }

    #[test]
    fn test_slot_margins() {
        assert_eq!(get_slot_margins_720("Rear", false, 84), (84, 84));
        assert_eq!(get_slot_margins_720("FrontFineArt", false, 84), (84, 84));
        assert_eq!(
            get_slot_margins_720("FrontPosterBoard", false, 84),
            (84, 566)
        );
        assert_eq!(
            get_slot_margins_720("FrontNoEjectRoller", false, 84),
            (84, 566)
        );
        assert_eq!(get_slot_margins_720("Rear", true, 84), (0, 0));
    }
}

