//! `epson-p900-media-install` — privileged helper (run as root via `pkexec`) that installs custom
//! media system-wide: `/etc/epson-p900-escpr-cups/custom_media.json`, ICC profiles in
//! `/usr/local/share/color/icc/epson-p900-custom/`, and refreshed PPDs of all queues of this driver.
//!
//! Usage:
//!   epson-p900-media-install <staging-dir>     install custom_media.json + icc/* as the media list
//!   epson-p900-media-install --remove <key>    remove one medium
//!   epson-p900-media-install --refresh-queues  only regenerate the queue PPDs
//!
//! SPDX-License-Identifier: GPL-3.0-or-later

use epson_p900_escpr_cups::system_media::{
    SystemPaths, install_staged_media, refresh_queue_ppds, remove_media,
};
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let paths = SystemPaths::system();
    let result = match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["--refresh-queues"] => refresh_queue_ppds(&paths.media_json),
        ["--remove", key] => remove_media(key, &paths).and_then(|mut r| {
            r.extend(refresh_queue_ppds(&paths.media_json)?);
            Ok(r)
        }),
        [dir] if !dir.starts_with('-') => {
            install_staged_media(Path::new(dir), &paths).and_then(|mut r| {
                r.extend(refresh_queue_ppds(&paths.media_json)?);
                Ok(r)
            })
        }
        _ => {
            eprintln!(
                "Usage: epson-p900-media-install <staging-dir> | --remove <key> | --refresh-queues"
            );
            std::process::exit(2);
        }
    };
    match result {
        Ok(report) => {
            for line in report {
                println!("{line}");
            }
        }
        Err(e) => {
            eprintln!("ERROR: {e}");
            std::process::exit(1);
        }
    }
}
