//! `rastertop900escpr` — CUPS Filter & Standalone CLI Driver for
//! Epson SureColor SC-P900 and SC-P700 (`ESC/P-R Level 5` + LittleCMS2).
//!
//! Modes of operation:
//! 1. **CUPS filter mode** (`argc == 6 || argc == 7`):
//!    `rastertop900escpr job-id user title copies options [file]`
//!    Streams CUPS Raster v2/v3 pages in 16-row bands with < 2 MB peak RAM,
//!    applies LittleCMS2 ICC color management per band, encodes `RHV2` `bsn2`
//!    packets, and writes the `ESC/P-R Level 5` stream to `stdout`.
//! 2. **Standalone CLI mode**:
//!    `rastertop900escpr --image photo.tif --media VelvetFineArt -o job.prn`
//!    `rastertop900escpr --test-strip --media PhotoPaperGlossy -o strip.prn`
//!    `rastertop900escpr --generate-ppd`
//!    `rastertop900escpr --generate-feature-table [--check]`
//!
//! SPDX-License-Identifier: GPL-3.0-or-later

mod cups_raster;
mod icc;
mod ppd;
mod protocol;
mod rhv2;

use cups_raster::CupsRasterReader;
use icc::{LcmsTransform, find_icc_profile};
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ImageDecoder, ImageReader, Rgb, RgbImage};
use protocol::{
    EscprJobConfig, build_job_footer, build_job_header, get_media_spec, get_paper_size,
    get_slot_margins_720, make_bsn2_packet, make_tsnd_packet,
};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};

const BAND_LINES: u32 = 16;

fn parse_cups_options(options_str: &str) -> HashMap<String, String> {
    let mut opts = HashMap::new();
    let tokens = shlex::split(options_str).unwrap_or_else(|| {
        options_str
            .split_whitespace()
            .map(ToString::to_string)
            .collect()
    });
    for token in tokens {
        if let Some((k, v)) = token.split_once('=') {
            opts.insert(k.to_string(), v.to_string());
        } else if let Some(rest) = token.strip_prefix("no") {
            opts.insert(rest.to_string(), "False".to_string());
        } else {
            opts.insert(token, "True".to_string());
        }
    }
    opts
}

fn parse_bool(val: &str) -> bool {
    matches!(
        val.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn build_config_from_cups_opts(
    opts: &HashMap<String, String>,
    job_user: &str,
    job_title: &str,
) -> (EscprJobConfig, String) {
    let media = opts
        .get("MediaType")
        .or_else(|| opts.get("media"))
        .cloned()
        .unwrap_or_else(|| "PlainPaper".to_string());
    let input_slot = opts
        .get("InputSlot")
        .cloned()
        .unwrap_or_else(|| "Rear".to_string());
    let page_size = opts
        .get("PageSize")
        .or_else(|| opts.get("media"))
        .cloned()
        .unwrap_or_else(|| "A4".to_string());
    let quality = opts
        .get("PrintQuality")
        .cloned()
        .unwrap_or_else(|| "High".to_string());
    let cm_calib = opts.get("cm-calibration").is_some_and(|v| parse_bool(v));
    let raw_color_mode = opts.get("EpsonColorMode").or_else(|| opts.get("ColorMode"));
    let color_mode = match raw_color_mode {
        None => {
            if cm_calib {
                "NoColorAdjustment".to_string()
            } else {
                "EpsonICC".to_string()
            }
        }
        Some(cm) if cm_calib && cm.eq_ignore_ascii_case("EpsonICC") => {
            "NoColorAdjustment".to_string()
        }
        Some(cm) => cm.clone(),
    };
    let intent = opts
        .get("EpsonRenderingIntent")
        .or_else(|| opts.get("RenderingIntent"))
        .cloned()
        .unwrap_or_else(|| "RelativeBPC".to_string());
    let overcoat = opts
        .get("EpsonOverCoat")
        .or_else(|| opts.get("OverCoat"))
        .cloned()
        .unwrap_or_else(|| "Auto".to_string());
    let borderless = opts.get("Borderless").is_some_and(|v| parse_bool(v));
    let unidirectional = opts.get("Unidirectional").is_some_and(|v| parse_bool(v));

    let user_media_id = opts
        .get("EpsonUserMediaID")
        .or_else(|| opts.get("UserMediaID"))
        .or_else(|| opts.get("CustomMediaSlot"))
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    let print_duty = opts
        .get("EpsonPrintDuty")
        .or_else(|| opts.get("PrintDuty"))
        .or_else(|| opts.get("ColorDensity"))
        .and_then(|v| {
            if v.eq_ignore_ascii_case("auto") {
                None
            } else {
                v.parse::<i8>().ok()
            }
        });
    let drying_time = opts
        .get("EpsonDryingTime")
        .or_else(|| opts.get("DryingTime"))
        .and_then(|v| {
            if v.eq_ignore_ascii_case("auto") {
                None
            } else {
                v.parse::<u8>().ok()
            }
        });
    let bottom_edge_priority = opts
        .get("EpsonBottomEdgePriority")
        .or_else(|| opts.get("BottomEdgePriority"))
        .and_then(|v| {
            if v.eq_ignore_ascii_case("auto") {
                None
            } else {
                Some(parse_bool(v))
            }
        });
    let rubbing_reduction = opts
        .get("EpsonRubbingReduction")
        .or_else(|| opts.get("RubbingReduction"))
        .and_then(|v| {
            if v.eq_ignore_ascii_case("auto") {
                None
            } else {
                v.parse::<u16>().ok()
            }
        });
    let ignore_panel_mismatch = opts
        .get("EpsonIgnorePanelMismatch")
        .or_else(|| opts.get("IgnorePanelMismatch"))
        .map_or(true, |v| parse_bool(v));

    let cfg = EscprJobConfig {
        media,
        input_slot,
        page_size,
        quality,
        color_mode,
        overcoat,
        borderless,
        unidirectional,
        margin_720: if borderless { 0 } else { 84 },
        custom_w_720: 0,
        custom_h_720: 0,
        job_name: job_title.to_string(),
        job_user: job_user.to_string(),
        job_host: "localhost".to_string(),
        user_media_id,
        print_duty,
        drying_time,
        bottom_edge_priority,
        rubbing_reduction,
        ignore_panel_mismatch,
    };
    (cfg, intent)
}

fn create_lcms_transform_if_needed(
    cfg: &EscprJobConfig,
    intent: &str,
    embedded_src_icc: Option<&[u8]>,
) -> Option<LcmsTransform> {
    if !cfg.color_mode.eq_ignore_ascii_case("EpsonICC") {
        return None;
    }
    let media_spec = get_media_spec(&cfg.media);
    let Some(icc_path) = find_icc_profile(&media_spec) else {
        eprintln!(
            "WARNING: [rastertop900escpr] Official Epson ICC profile '{}' not found! Passing sRGB unchanged.",
            media_spec.icc_profile
        );
        return None;
    };
    eprintln!(
        "INFO: [rastertop900escpr] Applying Epson ICC profile '{}' (intent={intent})",
        icc_path.display()
    );
    match LcmsTransform::new(&media_spec, intent, embedded_src_icc) {
        Ok(xform) => Some(xform),
        Err(e) => {
            eprintln!("WARNING: [rastertop900escpr] LittleCMS2 init failed ({e}); passing sRGB.");
            None
        }
    }
}

/// Run in CUPS filter mode, streaming scanlines in 16-row bands without buffering
/// the uncompressed page in memory, while still generating the 272-px LCD preview
/// thumbnail (`ESC t tsnd`).
fn run_cups_filter(args: &[String]) -> i32 {
    let job_user = args.get(2).map_or("linux", String::as_str);
    let job_title = args.get(3).map_or("CUPS_Job", String::as_str);
    let options_str = args.get(5).map_or("", String::as_str);
    let raster_file = args.get(6);

    let opts = parse_cups_options(options_str);
    let (mut cfg, intent) = build_config_from_cups_opts(&opts, job_user, job_title);

    let stdin = io::stdin();
    let reader: Box<dyn Read> = if let Some(path) = raster_file {
        match File::open(path) {
            Ok(f) => Box::new(BufReader::new(f)),
            Err(e) => {
                eprintln!("ERROR: [rastertop900escpr] Failed to open '{path}': {e}");
                return 1;
            }
        }
    } else {
        Box::new(BufReader::new(stdin.lock()))
    };

    let mut cups_reader = CupsRasterReader::new(reader);

    let stdout = io::stdout();
    let mut out = BufWriter::with_capacity(256 * 1024, stdout.lock());

    let mut page_idx = 0usize;
    loop {
        let Some(mut page) = (match cups_reader.next_page() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("ERROR: [rastertop900escpr] Error reading page header: {e}");
                return 1;
            }
        }) else {
            break;
        };
        page_idx += 1;

        let hdr = page.header.clone();
        if !hdr.media_type.is_empty() && cfg.media == "PlainPaper" {
            cfg.media = hdr.media_type.clone();
        }
        cfg.custom_w_720 = hdr.width;
        cfg.custom_h_720 = hdr.height;

        let xform = create_lcms_transform_if_needed(&cfg, &intent, None);

        let paper = get_paper_size(&cfg.page_size, hdr.width, hdr.height);
        let (margin_lr, margin_tb) =
            get_slot_margins_720(&cfg.input_slot, cfg.borderless, cfg.margin_720);
        let m_lr = u32::try_from(margin_lr.max(0)).unwrap_or(0);
        let m_tb = u32::try_from(margin_tb.max(0)).unwrap_or(0);

        let print_w = paper.width_720.saturating_sub(2 * m_lr);
        let print_h = paper.height_720.saturating_sub(2 * m_tb);

        eprintln!(
            "INFO: [rastertop900escpr] Page {page_idx}: media={} slot={} size={} ({print_w}x{print_h} @ 720dpi) quality={} color={}",
            cfg.media, cfg.input_slot, cfg.page_size, cfg.quality, cfg.color_mode
        );

        let src_w = hdr.width as usize;
        let src_h = hdr.height as usize;
        let dst_w = print_w as usize;
        let dst_h = print_h as usize;

        // Prepare tiny 272-px-tall rotated thumbnail buffer (e.g. 388x272 for A4, ~316 KB)
        let portrait_w = 272u32;
        let portrait_h = ((u64::from(print_h) * 272 + u64::from(print_w) / 2)
            / u64::from(print_w.max(1)))
        .max(16) as u32;
        let mut thumb_img = RgbImage::from_pixel(portrait_h, portrait_w, Rgb([255, 255, 255]));

        // If CUPS rendered the full page (including margins), crop by (m_lr, m_tb).
        let crop_x = if src_w > dst_w {
            (m_lr as usize).min(src_w - dst_w)
        } else {
            0
        };
        let crop_y = if src_h > dst_h {
            (m_tb as usize).min(src_h - dst_h)
        } else {
            0
        };

        let mut raw_row = vec![255u8; src_w * 3];
        // Skip top margin rows if present in the CUPS raster
        for _ in 0..crop_y {
            if let Err(e) = page.read_next_rgb_row(&mut raw_row) {
                eprintln!("ERROR: [rastertop900escpr] raster read error: {e}");
                return 1;
            }
        }
        let mut src_rows_consumed = crop_y;

        let band_lines = BAND_LINES as usize;
        let mut band_buf = vec![255u8; dst_w * band_lines * 3];
        let white_band_buf = vec![255u8; dst_w * band_lines * 3];

        // Queue of (iy_offset, lines) for consecutive all-white bands so trailing white
        // bands at the bottom of the page can be dropped without buffering pixel data.
        let mut pending_white_bands: Vec<(u32, usize)> = Vec::new();
        let mut compressed_bands: Vec<Vec<u8>> = Vec::new();

        let mut iy = 0usize;
        while iy < dst_h {
            let lines = band_lines.min(dst_h - iy);
            let active_len = dst_w * lines * 3;
            band_buf[..active_len].fill(255);

            for r in 0..lines {
                if src_rows_consumed < src_h {
                    if let Err(e) = page.read_next_rgb_row(&mut raw_row) {
                        eprintln!("ERROR: [rastertop900escpr] raster read error: {e}");
                        return 1;
                    }
                    src_rows_consumed += 1;
                    let copy_px = dst_w.min(src_w.saturating_sub(crop_x));
                    let src_slice = &raw_row[crop_x * 3..(crop_x + copy_px) * 3];
                    let dst_slice = &mut band_buf[r * dst_w * 3..(r * dst_w + copy_px) * 3];
                    dst_slice.copy_from_slice(src_slice);
                }

                // Sample into the rotated (portrait_h x portrait_w) thumbnail
                let cur_y = (iy + r) as u64;
                let py = ((cur_y * u64::from(portrait_h)) / u64::from(print_h.max(1)))
                    .min(u64::from(portrait_h - 1)) as u32;
                let prev_py = if cur_y == 0 {
                    u32::MAX
                } else {
                    (((cur_y - 1) * u64::from(portrait_h)) / u64::from(print_h.max(1)))
                        .min(u64::from(portrait_h - 1)) as u32
                };
                if py != prev_py {
                    let row_slice = &band_buf[r * dst_w * 3..(r + 1) * dst_w * 3];
                    for px in 0..portrait_w {
                        let sx = ((u64::from(px) * u64::from(print_w)) / u64::from(portrait_w))
                            .min(u64::from(print_w.saturating_sub(1)))
                            as usize;
                        let off = sx * 3;
                        let p = Rgb([row_slice[off], row_slice[off + 1], row_slice[off + 2]]);
                        // 90° CCW rotation: (px, py) -> (py, portrait_w - 1 - px)
                        thumb_img.put_pixel(py, portrait_w - 1 - px, p);
                    }
                }
            }

            let is_all_white = band_buf[..active_len].iter().all(|&b| b == 255);
            if is_all_white {
                pending_white_bands.push((iy as u32, lines));
            } else {
                // Flush any preceding interior white bands first
                for (w_iy, w_lines) in pending_white_bands.drain(..) {
                    compressed_bands.push(make_bsn2_packet(
                        &white_band_buf[..dst_w * w_lines * 3],
                        dst_w,
                        w_lines,
                        w_iy,
                        0,
                    ));
                }
                if let Some(ref xf) = xform {
                    xf.transform_rgb8_in_place(&mut band_buf[..active_len]);
                }
                compressed_bands.push(make_bsn2_packet(
                    &band_buf[..active_len],
                    dst_w,
                    lines,
                    iy as u32,
                    0,
                ));
            }

            iy += lines;
        }

        // Drain any remaining bottom margin rows from the CUPS page stream
        while src_rows_consumed < src_h {
            if page.read_next_rgb_row(&mut raw_row).is_err() {
                break;
            }
            src_rows_consumed += 1;
        }

        // Write Job Header + ESC t tsnd Preview + Compressed bsn2 Bands + Footer
        let job_hdr = build_job_header(&cfg, print_w, print_h);
        if let Err(e) = out.write_all(&job_hdr) {
            eprintln!("ERROR: [rastertop900escpr] stdout write error: {e}");
            return 1;
        }

        let mut jpeg_buf = Vec::with_capacity(16384);
        let mut enc = JpegEncoder::new_with_quality(&mut jpeg_buf, 95);
        if enc
            .encode(
                thumb_img.as_raw(),
                thumb_img.width(),
                thumb_img.height(),
                image::ExtendedColorType::Rgb8,
            )
            .is_ok()
        {
            let tsnd = make_tsnd_packet(&jpeg_buf);
            if let Err(e) = out.write_all(&tsnd) {
                eprintln!("ERROR: [rastertop900escpr] stdout write error: {e}");
                return 1;
            }
        }

        for pkt in &compressed_bands {
            if let Err(e) = out.write_all(pkt) {
                eprintln!("ERROR: [rastertop900escpr] stdout write error: {e}");
                return 1;
            }
        }

        if let Err(e) = out.write_all(&build_job_footer()) {
            eprintln!("ERROR: [rastertop900escpr] stdout write error: {e}");
            return 1;
        }
        let _ = out.flush();
    }

    0
}

/// Build the 272-px-tall rotated baseline JPEG LCD preview thumbnail (`388x272` for A4)
/// for `ESC t tsnd`.
fn build_preview_jpeg(page_rgb: &[u8], print_w: u32, print_h: u32) -> Option<Vec<u8>> {
    if print_w == 0 || print_h == 0 {
        return None;
    }
    let portrait_w = 272u32;
    let portrait_h = ((u64::from(print_h) * 272 + u64::from(print_w) / 2)
        / u64::from(print_w.max(1)))
    .max(16) as u32;

    // Downsample directly into a (portrait_h x portrait_w) rotated 90° CCW image
    // so we don't allocate a duplicate 50M-pixel DynamicImage.
    let mut small = RgbImage::new(portrait_h, portrait_w);
    for py in 0..portrait_h {
        let sy = ((u64::from(py) * u64::from(print_h)) / u64::from(portrait_h))
            .min(u64::from(print_h - 1)) as usize;
        for px in 0..portrait_w {
            let sx = ((u64::from(px) * u64::from(print_w)) / u64::from(portrait_w))
                .min(u64::from(print_w - 1)) as usize;
            let idx = (sy * (print_w as usize) + sx) * 3;
            let p = Rgb([page_rgb[idx], page_rgb[idx + 1], page_rgb[idx + 2]]);
            // Rotate 90° counter-clockwise: (px, py) -> (py, portrait_w - 1 - px)
            small.put_pixel(py, portrait_w - 1 - px, p);
        }
    }

    let mut jpeg_buf = Vec::with_capacity(16384);
    let mut enc = JpegEncoder::new_with_quality(&mut jpeg_buf, 95);
    if enc
        .encode(
            small.as_raw(),
            small.width(),
            small.height(),
            image::ExtendedColorType::Rgb8,
        )
        .is_ok()
    {
        Some(jpeg_buf)
    } else {
        None
    }
}

/// Generate the 10-patch diagnostic row (each patch <= 1.8 x 1.8 cm at 720 dpi)
/// across the top of the page.
fn generate_test_strip_image(print_w: u32) -> RgbImage {
    let patches: [(u8, u8, u8); 10] = [
        (0, 0, 0),       // PK/MK
        (110, 110, 110), // LK
        (190, 190, 190), // LLK
        (0, 255, 255),   // Cyan
        (255, 0, 255),   // Magenta
        (255, 255, 0),   // Yellow
        (255, 0, 0),     // Red
        (0, 255, 0),     // Green
        (0, 0, 255),     // Blue
        (96, 0, 255),    // Violet
    ];
    let patch_px = 510u32; // 18.0 mm at 720 dpi
    let gap_px = 56u32;
    let top_pad = 64u32;
    let bottom_pad = 32u32;
    let img_h = (top_pad + patch_px + bottom_pad).div_ceil(16) * 16;

    let mut img = RgbImage::from_pixel(print_w, img_h, Rgb([255, 255, 255]));
    let total_strip_w = (patches.len() as u32) * patch_px + ((patches.len() - 1) as u32) * gap_px;
    let x0_start = if print_w > total_strip_w {
        ((print_w - total_strip_w) / 2).max(16)
    } else {
        16
    };

    for (idx, &(r, g, b)) in patches.iter().enumerate() {
        let x0 = x0_start + (idx as u32) * (patch_px + gap_px);
        let x1 = (x0 + patch_px).min(print_w);
        let y0 = top_pad;
        let y1 = top_pad + patch_px;
        for y in y0..y1 {
            for x in x0..x1 {
                let is_border = x < x0 + 2 || x + 2 >= x1 || y < y0 + 2 || y + 2 >= y1;
                if is_border {
                    img.put_pixel(x, y, Rgb([80, 80, 80]));
                } else {
                    img.put_pixel(x, y, Rgb([r, g, b]));
                }
            }
        }
    }
    img
}

fn load_image_with_icc(path: &Path) -> Result<(RgbImage, Option<Vec<u8>>), String> {
    let reader = ImageReader::open(path)
        .map_err(|e| format!("Failed to open '{}': {e}", path.display()))?
        .with_guessed_format()
        .map_err(|e| format!("Failed to guess format of '{}': {e}", path.display()))?;
    let mut decoder = reader
        .into_decoder()
        .map_err(|e| format!("Failed to create decoder for '{}': {e}", path.display()))?;
    let icc = decoder.icc_profile().ok().flatten();
    let dyn_img = DynamicImage::from_decoder(decoder)
        .map_err(|e| format!("Failed to decode '{}': {e}", path.display()))?;
    Ok((dyn_img.to_rgb8(), icc))
}

#[allow(clippy::too_many_lines)]
fn run_cli(args: &[String]) -> i32 {
    let mut cfg = EscprJobConfig::default();
    let mut intent = "RelativeBPC".to_string();
    let mut image_path: Option<PathBuf> = None;
    let mut test_strip = false;
    let mut output_path: Option<PathBuf> = None;
    let mut send_addr: Option<String> = None;
    let mut gen_ppd = false;
    let mut ppd_dir = PathBuf::from("ppd");
    let mut gen_feature_table = false;
    let mut check_feature_table = false;

    let mut image_width_mm: Option<f64> = None;
    let mut image_height_mm: Option<f64> = None;
    let mut image_left_mm: Option<f64> = None;
    let mut image_top_mm: Option<f64> = None;
    let mut image_fit = "contain".to_string();
    let mut from_paper_edge = false;

    let mut i = 1usize;
    while i < args.len() {
        match args[i].as_str() {
            "--generate-ppd" => gen_ppd = true,
            "--ppd-dir" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    ppd_dir = PathBuf::from(v);
                }
            }
            "--generate-feature-table" => gen_feature_table = true,
            "--check" => check_feature_table = true,
            "--image" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    image_path = Some(PathBuf::from(v));
                }
            }
            "--test-strip" => test_strip = true,
            "--media" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    cfg.media.clone_from(v);
                }
            }
            "--input-slot" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    cfg.input_slot.clone_from(v);
                }
            }
            "--page-size" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    cfg.page_size.clone_from(v);
                }
            }
            "--quality" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    cfg.quality.clone_from(v);
                }
            }
            "--color-mode" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    cfg.color_mode.clone_from(v);
                }
            }
            "--intent" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    intent.clone_from(v);
                }
            }
            "--overcoat" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    cfg.overcoat.clone_from(v);
                }
            }
            "--borderless" => cfg.borderless = true,
            "--unidirectional" => cfg.unidirectional = true,
            "--user-media-id" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    cfg.user_media_id = v.parse().unwrap_or(0);
                }
            }
            "--print-duty" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    cfg.print_duty = v.parse().ok();
                }
            }
            "--drying-time" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    cfg.drying_time = v.parse().ok();
                }
            }
            "--bottom-edge-priority" => cfg.bottom_edge_priority = Some(true),
            "--rubbing-reduction" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    cfg.rubbing_reduction = v.parse().ok();
                }
            }
            "--verify-panel" => cfg.ignore_panel_mismatch = false,
            "--image-width-mm" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    image_width_mm = v.parse().ok();
                }
            }
            "--image-height-mm" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    image_height_mm = v.parse().ok();
                }
            }
            "--image-left-mm" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    image_left_mm = v.parse().ok();
                }
            }
            "--image-top-mm" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    image_top_mm = v.parse().ok();
                }
            }
            "--image-fit" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    image_fit.clone_from(v);
                }
            }
            "--from-paper-edge" => from_paper_edge = true,
            "-o" | "--output" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    output_path = Some(PathBuf::from(v));
                }
            }
            "--send" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    send_addr = Some(v.clone());
                }
            }
            "-h" | "--help" => {
                println!(
                    "Usage:\n  \
                     rastertop900escpr job-id user title copies options [file]   (CUPS filter mode)\n  \
                     rastertop900escpr --image <PATH> [options] -o <OUT.prn>\n  \
                     rastertop900escpr --test-strip [options] -o <OUT.prn>\n  \
                     rastertop900escpr --generate-ppd [--ppd-dir ppd]\n  \
                     rastertop900escpr --generate-feature-table [--check]"
                );
                return 0;
            }
            other => {
                eprintln!("Unknown option: {other}");
                return 2;
            }
        }
        i += 1;
    }

    if gen_ppd {
        if let Err(e) = ppd::generate_ppds(&ppd_dir) {
            eprintln!("ERROR generating PPDs: {e}");
            return 1;
        }
        return 0;
    }

    if gen_feature_table {
        let rendered = ppd::render_feature_table();
        let out_file = PathBuf::from("docs/supported-features.md");
        if check_feature_table {
            let current = std::fs::read_to_string(&out_file).unwrap_or_default();
            if current != rendered {
                eprintln!(
                    "{} is out of date; run `rastertop900escpr --generate-feature-table`",
                    out_file.display()
                );
                return 1;
            }
            println!("{} is up to date", out_file.display());
            return 0;
        }
        if let Some(parent) = out_file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(e) = std::fs::write(&out_file, rendered) {
            eprintln!("ERROR writing {}: {e}", out_file.display());
            return 1;
        }
        println!("Wrote {}", out_file.display());
        return 0;
    }

    if image_path.is_none() && !test_strip {
        eprintln!("Specify --image <PATH>, --test-strip, --generate-ppd, or run as a CUPS filter.");
        return 2;
    }

    if cfg.borderless {
        cfg.margin_720 = 0;
    }

    let paper = get_paper_size(&cfg.page_size, cfg.custom_w_720, cfg.custom_h_720);
    let (margin_lr, margin_tb) =
        get_slot_margins_720(&cfg.input_slot, cfg.borderless, cfg.margin_720);
    let m_lr = u32::try_from(margin_lr.max(0)).unwrap_or(0);
    let m_tb = u32::try_from(margin_tb.max(0)).unwrap_or(0);
    let print_w = paper.width_720.saturating_sub(2 * m_lr);
    let print_h = paper.height_720.saturating_sub(2 * m_tb);

    let (src_img, embedded_icc) = if let Some(ref p) = image_path {
        if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
            cfg.job_name = stem.to_string();
        }
        match load_image_with_icc(p) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("ERROR: {e}");
                return 1;
            }
        }
    } else {
        cfg.job_name = "TestStrip_18x18mm".to_string();
        (generate_test_strip_image(print_w), None)
    };

    let mm_to_px = |mm: f64| -> i64 { ((mm * 720.0) / 25.4).round() as i64 };
    let (iw, ih) = src_img.dimensions();
    let aspect = f64::from(iw) / f64::from(ih.max(1));

    let (target_w, target_h) = match (image_width_mm, image_height_mm) {
        (Some(w_mm), Some(h_mm)) => {
            let box_w = mm_to_px(w_mm).max(1) as u32;
            let box_h = mm_to_px(h_mm).max(1) as u32;
            if image_fit.eq_ignore_ascii_case("stretch") {
                (box_w, box_h)
            } else {
                let rw = f64::from(box_w) / f64::from(iw);
                let rh = f64::from(box_h) / f64::from(ih);
                let scale = if image_fit.eq_ignore_ascii_case("cover") {
                    rw.max(rh)
                } else {
                    rw.min(rh)
                };
                (
                    ((f64::from(iw) * scale).round() as u32).max(1),
                    ((f64::from(ih) * scale).round() as u32).max(1),
                )
            }
        }
        (Some(w_mm), None) => {
            let w_px = mm_to_px(w_mm).max(1) as u32;
            let h_px = ((f64::from(w_px) / aspect).round() as u32).max(1);
            (w_px, h_px)
        }
        (None, Some(h_mm)) => {
            let h_px = mm_to_px(h_mm).max(1) as u32;
            let w_px = ((f64::from(h_px) * aspect).round() as u32).max(1);
            (w_px, h_px)
        }
        (None, None) => (iw, ih),
    };

    let resized = if (target_w, target_h) == (iw, ih) {
        src_img
    } else {
        DynamicImage::ImageRgb8(src_img)
            .resize_exact(target_w, target_h, FilterType::Lanczos3)
            .to_rgb8()
    };

    let mut pos_x: i64 = if let Some(l_mm) = image_left_mm {
        let px = mm_to_px(l_mm);
        if from_paper_edge {
            px - i64::from(m_lr)
        } else {
            px
        }
    } else if test_strip {
        0
    } else {
        (i64::from(print_w) - i64::from(target_w)) / 2
    };

    let mut pos_y: i64 = if let Some(t_mm) = image_top_mm {
        let px = mm_to_px(t_mm);
        if from_paper_edge {
            px - i64::from(m_tb)
        } else {
            px
        }
    } else if test_strip {
        0
    } else {
        (i64::from(print_h) - i64::from(target_h)) / 2
    };

    // Place onto the full printable canvas (white background)
    let dst_w = print_w as usize;
    let dst_h = print_h as usize;
    let mut canvas = vec![255u8; dst_w * dst_h * 3];

    let src_x0 = (-pos_x).max(0) as u32;
    let src_y0 = (-pos_y).max(0) as u32;
    pos_x = pos_x.max(0);
    pos_y = pos_y.max(0);

    let copy_w = (target_w - src_x0).min(print_w.saturating_sub(pos_x as u32));
    let copy_h = (target_h - src_y0).min(print_h.saturating_sub(pos_y as u32));

    for r in 0..copy_h {
        let sy = (src_y0 + r) as usize;
        let dy = (pos_y as u32 + r) as usize;
        for c in 0..copy_w {
            let sx = (src_x0 + c) as usize;
            let dx = (pos_x as u32 + c) as usize;
            let p = resized.get_pixel(sx as u32, sy as u32);
            let off = (dy * dst_w + dx) * 3;
            canvas[off] = p[0];
            canvas[off + 1] = p[1];
            canvas[off + 2] = p[2];
        }
    }

    // 1. Build LCD preview thumbnail BEFORE ICC transform so the printer screen shows sRGB
    let preview_pkt = build_preview_jpeg(&canvas, print_w, print_h).map(|j| make_tsnd_packet(&j));

    // 2. Find last non-white band so trailing white space is not transmitted
    let band_lines = BAND_LINES as usize;
    let total_bands = dst_h.div_ceil(band_lines);
    let mut last_non_white_band = 0usize;
    for b_idx in 0..total_bands {
        let y0 = b_idx * band_lines;
        let lines = band_lines.min(dst_h - y0);
        let slice = &canvas[y0 * dst_w * 3..(y0 + lines) * dst_w * 3];
        if slice.iter().any(|&v| v != 255) {
            last_non_white_band = b_idx + 1;
        }
    }

    let xform = create_lcms_transform_if_needed(&cfg, &intent, embedded_icc.as_deref());

    let mut prn_stream = Vec::with_capacity(512 * 1024);
    prn_stream.extend_from_slice(&build_job_header(&cfg, print_w, print_h));
    if let Some(pkt) = preview_pkt {
        prn_stream.extend_from_slice(&pkt);
    }

    for b_idx in 0..last_non_white_band {
        let y0 = b_idx * band_lines;
        let lines = band_lines.min(dst_h - y0);
        let slice = &mut canvas[y0 * dst_w * 3..(y0 + lines) * dst_w * 3];
        if let Some(ref xf) = xform
            && slice.iter().any(|&v| v != 255)
        {
            xf.transform_rgb8_in_place(slice);
        }
        prn_stream.extend_from_slice(&make_bsn2_packet(slice, dst_w, lines, y0 as u32, 0));
    }
    prn_stream.extend_from_slice(&build_job_footer());

    if let Some(ref out_p) = output_path {
        if let Some(parent) = out_p.parent()
            && !parent.as_os_str().is_empty()
        {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(e) = std::fs::write(out_p, &prn_stream) {
            eprintln!("ERROR writing '{}': {e}", out_p.display());
            return 1;
        }
        println!(
            "Wrote {} bytes ({} bands) to {}",
            prn_stream.len(),
            last_non_white_band,
            out_p.display()
        );
    }

    if let Some(ref host_str) = send_addr {
        let target = if host_str.contains(':') {
            host_str.clone()
        } else {
            format!("{host_str}:9100")
        };
        println!("Sending {} bytes to {target} ...", prn_stream.len());
        match TcpStream::connect(&target) {
            Ok(mut sock) => {
                if let Err(e) = sock.write_all(&prn_stream) {
                    eprintln!("ERROR sending to {target}: {e}");
                    return 1;
                }
                let _ = sock.flush();
                println!("Sent successfully.");
            }
            Err(e) => {
                eprintln!("ERROR connecting to {target}: {e}");
                return 1;
            }
        }
    }

    if output_path.is_none() && send_addr.is_none() {
        let stdout = io::stdout();
        let mut out = stdout.lock();
        let _ = out.write_all(&prn_stream);
    }

    0
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let is_cups_filter =
        (args.len() == 6 || args.len() == 7) && !args[1].starts_with('-');
    let code = if is_cups_filter {
        run_cups_filter(&args)
    } else {
        run_cli(&args)
    };
    std::process::exit(code);
}
