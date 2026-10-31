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

/// System-wide custom media database, written by `epson-p900-media-install` (root) and read by the
/// CUPS filter (user `lp`), the PPD generator, LumaPrint and ep-media-installer.
pub const SYSTEM_CUSTOM_MEDIA_JSON: &str = "/etc/epson-p900-escpr-cups/custom_media.json";
/// Packaged (read-only) custom media list, lower priority than [`SYSTEM_CUSTOM_MEDIA_JSON`].
pub const PACKAGED_CUSTOM_MEDIA_JSON: &str = "/usr/share/epson-p900-escpr-cups/custom_media.json";
/// ICC profiles of custom media installed by `epson-p900-media-install`.
pub const SYSTEM_CUSTOM_ICC_DIR: &str = "/usr/local/share/color/icc/epson-p900-custom";

/// One entry of `custom_media.json` (unknown fields written by ep-media-installer are ignored).
#[derive(Debug, Clone, Deserialize)]
pub struct CustomMediaJsonEntry {
    #[serde(default)]
    pub slot: u8,
    #[serde(default)]
    pub user_media_id: u32,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub base_media: String,
    #[serde(default)]
    pub base_sn_id: Option<u8>,
    #[serde(default)]
    pub base_ppd_id: Option<u16>,
    #[serde(default)]
    pub icc_profile: String,
    #[serde(default)]
    pub win_icc: String,
    #[serde(default)]
    pub is_matte: Option<bool>,
    #[serde(default)]
    pub default_overcoat: Option<u8>,
    #[serde(default)]
    pub supports_carbon_black: Option<bool>,
    #[serde(default)]
    pub print_duty: i32,
    #[serde(default)]
    pub drying_time: u16,
    #[serde(default)]
    pub bottom_edge_priority: bool,
    #[serde(default)]
    pub rubbing_reduction: u16,
}

/// `custom_media.json` locations in priority order: `$EPSON_P900_CUSTOM_MEDIA_JSON` (development
/// override), then the system database and the packaged list. User home directories are not
/// searched: the CUPS filter runs as `lp` and could not read them anyway.
pub fn custom_media_json_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(env_p) = std::env::var("EPSON_P900_CUSTOM_MEDIA_JSON")
        && !env_p.trim().is_empty()
    {
        paths.push(PathBuf::from(env_p));
    }
    paths.push(PathBuf::from(SYSTEM_CUSTOM_MEDIA_JSON));
    paths.push(PathBuf::from(PACKAGED_CUSTOM_MEDIA_JSON));
    paths
}

pub struct MediaRegistry {
    pub specs: Vec<MediaSpec>,
    pub by_key: HashMap<String, usize>,
}

pub fn media_registry() -> &'static MediaRegistry {
    static REGISTRY: OnceLock<MediaRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| MediaRegistry::from_json_paths(&custom_media_json_candidates()))
}

impl MediaRegistry {
    /// Built-in media plus the custom media of the given `custom_media.json` files (earlier files
    /// win on duplicate keys; missing or unparsable files are skipped).
    pub fn from_json_paths(paths: &[PathBuf]) -> Self {
        let mut specs = builtin_media_specs();
        let mut by_key = HashMap::new();
        for (idx, m) in specs.iter().enumerate() {
            by_key.insert(m.key.to_ascii_lowercase(), idx);
            by_key.insert(m.ppd_id.to_string(), idx);
            by_key.insert(format!("sn_{}", m.sn_id), idx);
        }

        for path in paths {
            let Ok(text) = std::fs::read_to_string(path) else {
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
    }

    /// The medium for a CUPS `MediaType` value (key, PPD id, `sn_<id>`, slot alias), or `None`.
    pub fn find(&self, media_option: &str) -> Option<MediaSpec> {
        let key = media_option.trim().to_ascii_lowercase();
        self.by_key.get(&key).map(|&idx| self.specs[idx].clone())
    }
}

/// The medium for a CUPS `MediaType` value, or `None` if it is neither built in nor installed.
pub fn find_media_spec(media_option: &str) -> Option<MediaSpec> {
    media_registry().find(media_option)
}

/// Like [`find_media_spec`], falling back to plain paper (for PPD generation and table lookups).
pub fn get_media_spec(media_option: &str) -> MediaSpec {
    find_media_spec(media_option).unwrap_or_else(|| {
        let reg = media_registry();
        let plain_idx = reg.by_key.get("plainpaper").copied().unwrap_or(0);
        reg.specs[plain_idx].clone()
    })
}

pub fn resolve_user_media_id(raw_id: u32, media: &MediaSpec) -> u32 {
    media_registry().resolve_user_media_id(raw_id, media)
}

impl MediaRegistry {
    /// Resolves slot numbers `1..=30` and UI IDs `1901..=1930` to the slot's 32-bit `MA_ID`;
    /// other values (already 32-bit IDs) pass through.
    pub fn resolve_user_media_id(&self, raw_id: u32, media: &MediaSpec) -> u32 {
        let uid = if raw_id > 0 {
            raw_id
        } else {
            media.user_media_id
        };
        let reg = self;
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

pub fn resolve_firmware_print_quality(
    media: &MediaSpec,
    quality_key: &str,
    overcoat_val: u8,
) -> u8 {
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

pub fn get_slot_margins_720(
    input_slot: &str,
    borderless: bool,
    default_margin_720: i16,
) -> (i16, i16) {
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

/// Advanced B&W Photo settings, sent as `ESC q setb` and mirrored in the REMOTE1 `HD` block.
/// Layout and value ranges verified against Epson Print Layout captures
/// (`epson-p900-analyse/epl_reference/captures/EINSTELLUNGEN.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbwSettings {
    /// Toning wheel, horizontal axis (`-75..=75`; Cool preset -8, Warm preset 4).
    pub tone_h: i32,
    /// Toning wheel, vertical axis (`-75..=75`; Cool preset -45, Warm preset 20).
    pub tone_v: i32,
    /// Mid-gray tone: 0 Light, 1 Normal, 2 Dark, 3 Darker (EPL default), 4 Darkest.
    pub midtone: u8,
    pub brightness: i32,
    pub contrast: i32,
    pub shadow_tonality: i32,
    pub highlight_tonality: i32,
    /// Max optical density (`-50..=0`).
    pub max_density: i32,
    /// Highlight point shift: 0 off, 1 image area, 2 full page.
    pub highlight_shift: u8,
}

impl Default for AbwSettings {
    fn default() -> Self {
        Self {
            tone_h: 0,
            tone_v: 0,
            midtone: 3,
            brightness: 0,
            contrast: 0,
            shadow_tonality: 0,
            highlight_tonality: 0,
            max_density: 0,
            highlight_shift: 0,
        }
    }
}

pub const ABW_MIDTONES: [&str; 5] = ["Light", "Normal", "Dark", "Darker", "Darkest"];
pub const ABW_HIGHLIGHT_SHIFTS: [&str; 3] = ["Off", "ImageArea", "FullPage"];
/// Toning presets with their wheel coordinates as set by Epson Print Layout.
pub const ABW_TONE_PRESETS: [(&str, i32, i32); 3] =
    [("Neutral", 0, 0), ("Cool", -8, -45), ("Warm", 4, 20)];

impl AbwSettings {
    fn clamped(&self) -> Self {
        Self {
            tone_h: self.tone_h.clamp(-75, 75),
            tone_v: self.tone_v.clamp(-75, 75),
            midtone: self.midtone.min(4),
            brightness: self.brightness.clamp(-25, 25),
            contrast: self.contrast.clamp(-25, 25),
            shadow_tonality: self.shadow_tonality.clamp(-25, 25),
            highlight_tonality: self.highlight_tonality.clamp(-25, 25),
            max_density: self.max_density.clamp(-50, 0),
            highlight_shift: self.highlight_shift.min(2),
        }
    }

    /// Applies one option (`Tone`, `ToneH`, `ToneV`, `Midtone`, `Brightness`, `Contrast`,
    /// `ShadowTone`, `HighlightTone`, `MaxDensity`, `HighlightShift`; case-insensitive, optional
    /// `EpsonAbw` prefix). Returns `false` for unknown keys or unparsable values.
    pub fn apply_option(&mut self, key: &str, value: &str) -> bool {
        let lower = key.to_ascii_lowercase();
        let k = lower.strip_prefix("epsonabw").unwrap_or(&lower);
        let v = value.trim();
        let int = || v.trim_start_matches('+').parse::<i32>().ok();
        let index_of = |names: &[&str]| {
            names
                .iter()
                .position(|n| n.eq_ignore_ascii_case(v))
                .and_then(|i| u8::try_from(i).ok())
                .or_else(|| v.parse::<u8>().ok())
        };
        let target = match k {
            "tone" => {
                let Some((_, h, vv)) = ABW_TONE_PRESETS
                    .iter()
                    .find(|(n, _, _)| n.eq_ignore_ascii_case(v))
                else {
                    return false;
                };
                self.tone_h = *h;
                self.tone_v = *vv;
                return true;
            }
            "midtone" => {
                return index_of(&ABW_MIDTONES).is_some_and(|m| {
                    self.midtone = m;
                    true
                });
            }
            "highlightshift" => {
                return index_of(&ABW_HIGHLIGHT_SHIFTS).is_some_and(|m| {
                    self.highlight_shift = m;
                    true
                });
            }
            "toneh" => &mut self.tone_h,
            "tonev" => &mut self.tone_v,
            "brightness" => &mut self.brightness,
            "contrast" => &mut self.contrast,
            "shadowtone" => &mut self.shadow_tonality,
            "highlighttone" => &mut self.highlight_tonality,
            "maxdensity" => &mut self.max_density,
            _ => return false,
        };
        int().is_some_and(|n| {
            *target = n;
            true
        })
    }

    /// `ESC q setb` payload: nine big-endian two's-complement `i32`.
    pub fn setb_payload(&self) -> [u8; 36] {
        let a = self.clamped();
        let vals = [
            a.tone_h,
            a.tone_v,
            160 + 20 * i32::from(a.midtone),
            a.brightness,
            a.contrast,
            a.shadow_tonality,
            a.highlight_tonality,
            a.max_density,
            i32::from(a.highlight_shift),
        ];
        let mut out = [0u8; 36];
        for (chunk, v) in out.chunks_exact_mut(4).zip(vals) {
            chunk.copy_from_slice(&v.to_be_bytes());
        }
        out
    }
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
    pub abw: AbwSettings,
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
            abw: AbwSettings::default(),
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

    let is_abw = lut_byte == 7;
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
    if is_abw {
        hd.extend_from_slice(&tlv_str(0x04, Some(1), "Advanced B&W Photo"));
    } else {
        hd.extend_from_slice(&tlv_str(0x04, Some(0), "Color"));
    }
    hd.extend_from_slice(&tlv_str(0x05, Some(q_val), q_label));
    hd.extend_from_slice(&tlv_str(
        0x06,
        Some(if overcoat_on { 1 } else { 0 }),
        if overcoat_on { "On" } else { "Off" },
    ));
    if is_abw {
        // Value byte = setting + offset, text = setting (same tag order as Epson Print Layout).
        let a = cfg.abw.clamped();
        let num = |tag: u8, v: i32, offset: i32| {
            let vb = u8::try_from(v + offset).unwrap_or(0);
            tlv_str(tag, Some(vb), &v.to_string())
        };
        let preset = ABW_TONE_PRESETS
            .iter()
            .position(|(_, h, v)| *h == a.tone_h && *v == a.tone_v);
        let (preset_val, preset_str) = match preset {
            Some(0) => (0u8, "Neutral"),
            Some(2) => (2u8, "Warm"),
            _ => (0x80u8, "Custom"),
        };
        hd.extend_from_slice(&num(0x14, a.brightness, 25));
        hd.extend_from_slice(&num(0x15, a.contrast, 25));
        hd.extend_from_slice(&tlv_str(0x12, Some(preset_val), preset_str));
        hd.extend_from_slice(&tlv_str(
            0x13,
            Some(a.midtone),
            ABW_MIDTONES[usize::from(a.midtone)],
        ));
        hd.extend_from_slice(&num(0x16, a.shadow_tonality, 25));
        hd.extend_from_slice(&num(0x17, a.tone_h, 75));
        hd.extend_from_slice(&num(0x18, a.tone_v, 75));
        hd.extend_from_slice(&num(0x19, a.highlight_tonality, 25));
        hd.extend_from_slice(&num(0x1A, a.max_density, 50));
        hd.extend_from_slice(&tlv_str(
            0x1B,
            Some(a.highlight_shift),
            ["Off", "On", "On (Full Page)"][usize::from(a.highlight_shift)],
        ));
    } else {
        hd.extend_from_slice(&tlv_str(0x07, Some(lut_byte), lut_str));
        hd.extend_from_slice(&tlv_str(0x08, Some(1), "2.2"));
        hd.extend_from_slice(&tlv_str(0x09, Some(0x19), "0"));
        hd.extend_from_slice(&tlv_str(0x0A, Some(0x19), "0"));
        hd.extend_from_slice(&tlv_str(0x0B, Some(0x19), "0"));
        hd.extend_from_slice(&tlv_str(0x17, Some(0x4B), "0"));
        hd.extend_from_slice(&tlv_str(0x18, Some(0x4B), "0"));
    }
    hd.extend_from_slice(&tlv_str(0x1C, Some(res_id), res_str));
    hd.extend_from_slice(&tlv_str(0x1E, Some(0), "Off"));
    hd
}

pub fn build_job_header(cfg: &EscprJobConfig, print_w: u32, print_h: u32) -> Vec<u8> {
    build_job_header_with(media_registry(), cfg, print_w, print_h)
}

/// [`build_job_header`] with an explicit media registry.
pub fn build_job_header_with(
    registry: &MediaRegistry,
    cfg: &EscprJobConfig,
    print_w: u32,
    print_h: u32,
) -> Vec<u8> {
    let media = registry.find(&cfg.media).unwrap_or_else(|| {
        let plain_idx = registry.by_key.get("plainpaper").copied().unwrap_or(0);
        registry.specs[plain_idx].clone()
    });
    let paper = get_paper_size(&cfg.page_size, cfg.custom_w_720, cfg.custom_h_720);
    let slot = get_input_slot(&cfg.input_slot);

    let overcoat_val = if cfg.overcoat.eq_ignore_ascii_case("on") && !media.is_matte {
        2u8
    } else if cfg.overcoat.eq_ignore_ascii_case("off") {
        1u8
    } else {
        media.default_overcoat
    };

    let eff_user_media_id = registry.resolve_user_media_id(cfg.user_media_id, &media);
    let eff_print_duty = cfg.print_duty.unwrap_or(media.print_duty).clamp(-50, 50);
    let eff_drying = cfg.drying_time.unwrap_or(media.drying_time);
    let eff_bottom_edge = cfg
        .bottom_edge_priority
        .unwrap_or(media.bottom_edge_priority);
    let eff_rubbing = cfg.rubbing_reduction.unwrap_or(media.rubbing_reduction);

    let q_val = resolve_firmware_print_quality(&media, &cfg.quality, overcoat_val);

    // The color byte of `ESC q setq` / `ESC p setq` stays 0 in every mode, including Advanced B&W:
    // Epson Print Layout selects ABW through LUT 7 plus `ESC q setb`.
    let is_abw = cfg.color_mode.eq_ignore_ascii_case("advancedbw");
    let lut_byte = if is_abw {
        7u8
    } else if cfg.color_mode.eq_ignore_ascii_case("epsonadobergb")
        || cfg.color_mode.eq_ignore_ascii_case("adobergb")
    {
        6u8
    } else if cfg.color_mode.eq_ignore_ascii_case("epsonstandard")
        || cfg.color_mode.eq_ignore_ascii_case("srgb")
    {
        0u8
    } else {
        4u8
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

    let hd_payload = build_remote1_hd_payload(cfg, &media, &paper, q_val, overcoat_val, lut_byte);

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
    let q_setq = [media.sn_id, q_val, 0, 0, 0, 0, 3, 0, 0];
    out.extend_from_slice(&make_escpr_packet(b'q', b"setq", &q_setq));

    // 4. ESC q seti (8 bytes)
    let q_seti = [1, 0, 0, 0, 0, 0, overcoat_val, 0];
    out.extend_from_slice(&make_escpr_packet(b'q', b"seti", &q_seti));

    // 4b. ESC q setb (36 bytes, Advanced B&W Photo parameters)
    if is_abw {
        out.extend_from_slice(&make_escpr_packet(b'q', b"setb", &cfg.abw.setb_payload()));
    }

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
    let (margin_lr, margin_tb) =
        get_slot_margins_720(&cfg.input_slot, cfg.borderless, cfg.margin_720);
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
        let json =
            std::env::temp_dir().join(format!("epson-p900-header-{}.json", std::process::id()));
        std::fs::write(
            &json,
            r#"[{"slot": 1, "user_media_id": 915907607, "key": "Slot01_Example_Luster",
                 "name": "Example Luster", "base_media": "PremiumLuster", "base_sn_id": 13}]"#,
        )
        .unwrap();
        let reg = MediaRegistry::from_json_paths(std::slice::from_ref(&json));
        let _ = std::fs::remove_file(&json);
        let m = reg.find("Slot01_Example_Luster").unwrap();
        assert_eq!(m.slot, 1);
        assert_eq!(m.sn_id, 13); // PremiumLuster
        assert_eq!(m.user_media_id, 915_907_607);
        assert_eq!(reg.resolve_user_media_id(1, &m), 915_907_607);
        assert_eq!(reg.resolve_user_media_id(1901, &m), 915_907_607);

        let cfg = EscprJobConfig {
            media: "Slot01_Example_Luster".to_string(),
            print_duty: Some(-10),
            bottom_edge_priority: Some(true),
            ..Default::default()
        };
        let hdr = build_job_header_with(&reg, &cfg, 5784, 8250);
        // Verify ESC q setl packet: 1b 'q' 04 00 00 00 "setl" 36 97 a4 17
        let expected_setl = b"\x1bq\x04\x00\x00\x00setl\x36\x97\xa4\x17";
        assert!(
            hdr.windows(expected_setl.len()).any(|w| w == expected_setl),
            "ESC q setl with MA_ID 0x3697a417 not found in job header"
        );
        // Verify ESC m seti packet: 1b 'm' 04 00 00 00 "seti" 00 00 00 01
        let expected_seti = b"\x1bm\x04\x00\x00\x00seti\x00\x00\x00\x01";
        assert!(
            hdr.windows(expected_seti.len()).any(|w| w == expected_seti),
            "ESC m seti not found in job header"
        );
    }

    #[test]
    fn test_abw_setb_matches_epson_print_layout() {
        // Capture AdvancedBW-A: Cool (-8/-45), Light, +5, -7, +9, -11, -13, highlight shift off.
        let mut abw = AbwSettings::default();
        for (k, v) in [
            ("EpsonAbwTone", "Cool"),
            ("Midtone", "Light"),
            ("Brightness", "+5"),
            ("Contrast", "-7"),
            ("ShadowTone", "9"),
            ("HighlightTone", "-11"),
            ("MaxDensity", "-13"),
        ] {
            assert!(abw.apply_option(k, v), "{k}={v}");
        }
        let expected_a: [u8; 36] = [
            0xff, 0xff, 0xff, 0xf8, 0xff, 0xff, 0xff, 0xd3, 0, 0, 0, 0xa0, 0, 0, 0, 0x05, 0xff,
            0xff, 0xff, 0xf9, 0, 0, 0, 0x09, 0xff, 0xff, 0xff, 0xf5, 0xff, 0xff, 0xff, 0xf3, 0, 0,
            0, 0,
        ];
        assert_eq!(abw.setb_payload(), expected_a);
        assert!(!abw.apply_option("Midtone", "Medium"));
        assert!(!abw.apply_option("Unknown", "1"));

        // Default = EPL "Neutral / Darker"; header uses color byte 0, LUT 7 and sends setb.
        let cfg = EscprJobConfig {
            media: "PhotoPaperGlossy".to_string(),
            color_mode: "AdvancedBW".to_string(),
            ..Default::default()
        };
        let hdr = build_job_header(&cfg, 5784, 8250);
        let mut setb = b"\x1bq\x24\x00\x00\x00setb".to_vec();
        setb.extend_from_slice(&[0u8; 11]);
        setb.push(0xdc);
        setb.extend_from_slice(&[0u8; 24]);
        assert!(hdr.windows(setb.len()).any(|w| w == setb.as_slice()));
        let p_setq = b"\x1bp\x0c\x00\x00\x00setq\x00\x03\x07\x00\xdc";
        assert!(hdr.windows(p_setq.len()).any(|w| w == p_setq));
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
