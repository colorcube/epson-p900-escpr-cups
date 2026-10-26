//! LittleCMS2 (`liblcms2`) FFI wrapper and ICC profile locator for the
//! Epson SureColor SC-P900 and SC-P700.
//!
//! Uses the exact same `liblcms2.so.2` color engine (`Relative Colorimetric + BPC`
//! with `cmsFLAGS_HIGHRESPRECALC`) verified on hardware against Epson Print Layout.
//!
//! SPDX-License-Identifier: GPL-3.0-or-later

use crate::protocol::MediaSpec;
use std::ffi::{CString, c_char, c_int, c_uint, c_void};
use std::path::{Path, PathBuf};
use thiserror::Error;

const TYPE_RGB_8: u32 = 0x0004_0019;

const INTENT_PERCEPTUAL: u32 = 0;
const INTENT_RELATIVE_COLORIMETRIC: u32 = 1;
const INTENT_SATURATION: u32 = 2;
const INTENT_ABSOLUTE_COLORIMETRIC: u32 = 3;

const CMS_FLAGS_HIGHRESPRECALC: u32 = 0x0400;
const CMS_FLAGS_BLACKPOINTCOMPENSATION: u32 = 0x2000;

#[link(name = "lcms2")]
unsafe extern "C" {
    fn cmsOpenProfileFromFile(icc_profile: *const c_char, access: *const c_char) -> *mut c_void;
    fn cmsOpenProfileFromMem(mem_ptr: *const c_void, size: c_uint) -> *mut c_void;
    fn cmsCreate_sRGBProfile() -> *mut c_void;
    fn cmsCloseProfile(h_profile: *mut c_void) -> c_int;
    fn cmsCreateTransform(
        input: *mut c_void,
        input_format: u32,
        output: *mut c_void,
        output_format: u32,
        intent: u32,
        flags: u32,
    ) -> *mut c_void;
    fn cmsDeleteTransform(h_transform: *mut c_void);
    fn cmsDoTransform(
        transform: *mut c_void,
        input_buffer: *const c_void,
        output_buffer: *mut c_void,
        size: c_uint,
    );
}

#[derive(Debug, Error)]
pub enum IccError {
    #[error(
        "could not find Epson ICC profile for media '{0}' (run './icc/fetch_icc_profiles.sh' first)"
    )]
    ProfileNotFound(String),
    #[error("failed to open ICC profile '{0}' via LittleCMS2")]
    OpenFailed(String),
    #[error("failed to create LittleCMS2 RGB transform")]
    TransformCreationFailed,
}

pub fn default_icc_search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(env_dir) = std::env::var("EPSON_P900_ICC_DIR")
        && !env_dir.trim().is_empty()
    {
        dirs.push(PathBuf::from(env_dir));
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        dirs.push(parent.join("icc"));
        if let Some(grand) = parent.parent() {
            dirs.push(grand.join("icc"));
            if let Some(root) = grand.parent() {
                dirs.push(root.join("icc"));
            }
        }
    }
    dirs.push(PathBuf::from("icc"));
    if let Ok(home) = std::env::var("HOME") {
        let h = PathBuf::from(home);
        dirs.push(h.join(".local/share/color/icc/epson-scp900"));
        dirs.push(h.join(".local/share/color/icc"));
    }
    dirs.push(PathBuf::from("/usr/share/color/icc/epson-scp900"));
    dirs.push(PathBuf::from("/usr/local/share/color/icc/epson-scp900"));
    dirs.push(PathBuf::from("/usr/share/color/icc"));
    dirs
}

pub fn find_icc_profile(media: &MediaSpec) -> Option<PathBuf> {
    if !media.icc_profile.is_empty() {
        let direct = PathBuf::from(&media.icc_profile);
        if direct.is_file() {
            return Some(direct);
        }
    }

    let base_name = Path::new(&media.icc_profile)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let win_upper = media.win_icc.clone();
    let win_lower = media.win_icc.to_ascii_lowercase();
    let candidates = [base_name.as_str(), win_upper.as_str(), win_lower.as_str()];

    for dir in default_icc_search_dirs() {
        if !dir.is_dir() {
            continue;
        }
        for cand in candidates {
            if cand.is_empty() {
                continue;
            }
            let p = dir.join(cand);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

pub fn resolve_intent_and_flags(rendering_intent: &str) -> (u32, u32) {
    let ri = rendering_intent.trim().to_ascii_lowercase();
    match ri.as_str() {
        "perceptual" | "0" => (
            INTENT_PERCEPTUAL,
            CMS_FLAGS_BLACKPOINTCOMPENSATION | CMS_FLAGS_HIGHRESPRECALC,
        ),
        "relative" | "relativenobpc" | "relativecolorimetricnobpc" => {
            (INTENT_RELATIVE_COLORIMETRIC, CMS_FLAGS_HIGHRESPRECALC)
        }
        "saturation" | "2" => (INTENT_SATURATION, CMS_FLAGS_HIGHRESPRECALC),
        "absolute" | "absolutecolorimetric" | "3" => {
            (INTENT_ABSOLUTE_COLORIMETRIC, CMS_FLAGS_HIGHRESPRECALC)
        }
        _ => (
            INTENT_RELATIVE_COLORIMETRIC,
            CMS_FLAGS_BLACKPOINTCOMPENSATION | CMS_FLAGS_HIGHRESPRECALC,
        ),
    }
}

/// Reusable LittleCMS2 RGB-to-RGB 8-bit transform for band-wise streaming.
pub struct LcmsTransform {
    handle: *mut c_void,
}

unsafe impl Send for LcmsTransform {}

impl LcmsTransform {
    pub fn new(
        media: &MediaSpec,
        rendering_intent: &str,
        embedded_src_icc: Option<&[u8]>,
    ) -> Result<Self, IccError> {
        let dst_path =
            find_icc_profile(media).ok_or_else(|| IccError::ProfileNotFound(media.key.clone()))?;
        let dst_cstr = CString::new(dst_path.to_string_lossy().as_bytes())
            .map_err(|_| IccError::OpenFailed(dst_path.display().to_string()))?;
        let mode_r = CString::new("r").expect("static cstring");

        unsafe {
            let dst_prof = cmsOpenProfileFromFile(dst_cstr.as_ptr(), mode_r.as_ptr());
            if dst_prof.is_null() {
                return Err(IccError::OpenFailed(dst_path.display().to_string()));
            }

            let src_prof = if let Some(icc_bytes) = embedded_src_icc
                && !icc_bytes.is_empty()
            {
                let p = cmsOpenProfileFromMem(
                    icc_bytes.as_ptr().cast::<c_void>(),
                    icc_bytes.len() as c_uint,
                );
                if p.is_null() {
                    cmsCreate_sRGBProfile()
                } else {
                    p
                }
            } else {
                cmsCreate_sRGBProfile()
            };

            if src_prof.is_null() {
                cmsCloseProfile(dst_prof);
                return Err(IccError::OpenFailed("sRGB built-in".to_string()));
            }

            let (intent, flags) = resolve_intent_and_flags(rendering_intent);
            let xform = cmsCreateTransform(
                src_prof,
                TYPE_RGB_8,
                dst_prof,
                TYPE_RGB_8,
                intent,
                flags,
            );
            cmsCloseProfile(src_prof);
            cmsCloseProfile(dst_prof);

            if xform.is_null() {
                return Err(IccError::TransformCreationFailed);
            }
            Ok(Self { handle: xform })
        }
    }

    /// Transform an interleaved 24-bit RGB buffer (`len == num_pixels * 3`) in-place.
    pub fn transform_rgb8_in_place(&self, rgb_buf: &mut [u8]) {
        let num_pixels = (rgb_buf.len() / 3) as c_uint;
        if num_pixels == 0 {
            return;
        }
        unsafe {
            cmsDoTransform(
                self.handle,
                rgb_buf.as_ptr().cast::<c_void>(),
                rgb_buf.as_mut_ptr().cast::<c_void>(),
                num_pixels,
            );
        }
    }
}

impl Drop for LcmsTransform {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                cmsDeleteTransform(self.handle);
            }
        }
    }
}
