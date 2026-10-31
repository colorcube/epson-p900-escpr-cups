//! Shared library of the `epson-p900-escpr-cups` driver: ESC/P-R protocol, CUPS raster reader,
//! LittleCMS2 ICC handling, RHV2 compression and PPD generation. Used by the
//! `rastertop900escpr` filter/CLI and the privileged `epson-p900-media-install` helper.
//!
//! SPDX-License-Identifier: GPL-3.0-or-later

pub mod cups_raster;
pub mod icc;
pub mod ppd;
pub mod protocol;
pub mod rhv2;
pub mod system_media;
