//! System-wide custom media installation, used by the privileged `epson-p900-media-install`
//! helper (run as root via `pkexec`, see the polkit policy in `share/polkit-1/actions/`).
//!
//! The helper is what Epson's "Convert Tool" does on Windows/macOS: it turns the media list prepared
//! by ep-media-installer (a staging directory owned by the user) into the system database
//! [`SYSTEM_CUSTOM_MEDIA_JSON`], installs the ICC profiles into [`SYSTEM_CUSTOM_ICC_DIR`] and
//! regenerates the PPDs of all CUPS queues that use this driver.
//!
//! Everything read from the staging directory is untrusted: files are opened without following
//! symlinks, size-limited, and must parse as JSON / carry the ICC `acsp` signature.
//!
//! SPDX-License-Identifier: GPL-3.0-or-later

use crate::ppd::{build_ppd_with, merge_ppd_defaults};
use crate::protocol::{
    CustomMediaJsonEntry, MediaRegistry, PACKAGED_CUSTOM_MEDIA_JSON, SYSTEM_CUSTOM_ICC_DIR,
    SYSTEM_CUSTOM_MEDIA_JSON, builtin_media_specs,
};
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;

const MAX_JSON_BYTES: u64 = 1024 * 1024;
const MAX_ICC_BYTES: u64 = 32 * 1024 * 1024;
const BUILTIN_ICC_DIR: &str = "/usr/share/color/icc/epson-scp900";

/// Target locations; [`SystemPaths::system`] in production, a temporary root in tests.
#[derive(Debug, Clone)]
pub struct SystemPaths {
    pub media_json: PathBuf,
    pub icc_dir: PathBuf,
    pub builtin_icc_dir: PathBuf,
}

impl SystemPaths {
    pub fn system() -> Self {
        Self {
            media_json: PathBuf::from(SYSTEM_CUSTOM_MEDIA_JSON),
            icc_dir: PathBuf::from(SYSTEM_CUSTOM_ICC_DIR),
            builtin_icc_dir: PathBuf::from(BUILTIN_ICC_DIR),
        }
    }

    /// All paths below `root` (for tests and `--root`).
    pub fn under_root(root: &Path) -> Self {
        let rebase = |p: &str| root.join(p.trim_start_matches('/'));
        Self {
            media_json: rebase(SYSTEM_CUSTOM_MEDIA_JSON),
            icc_dir: rebase(SYSTEM_CUSTOM_ICC_DIR),
            builtin_icc_dir: rebase(BUILTIN_ICC_DIR),
        }
    }
}

/// Reads a regular file without following a symlink at the final path component.
pub fn read_untrusted_file(path: &Path, max_bytes: u64) -> Result<Vec<u8>, String> {
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let meta = file
        .metadata()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if !meta.is_file() {
        return Err(format!("{}: not a regular file", path.display()));
    }
    if meta.len() > max_bytes {
        return Err(format!("{}: larger than {max_bytes} bytes", path.display()));
    }
    let mut data = Vec::with_capacity(usize::try_from(meta.len()).unwrap_or(0));
    file.take(max_bytes + 1)
        .read_to_end(&mut data)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(data)
}

pub fn is_icc_profile(bytes: &[u8]) -> bool {
    bytes.len() >= 132 && bytes.get(36..40) == Some(b"acsp".as_slice())
}

fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 32
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

/// ICC file names are reduced to a safe basename (`[A-Za-z0-9._-]`, `.icc`/`.icm`).
fn safe_icc_name(raw: &str) -> Option<String> {
    let base = Path::new(raw).file_name()?.to_str()?;
    let lower = base.to_ascii_lowercase();
    let ok = (lower.ends_with(".icc") || lower.ends_with(".icm"))
        && base.len() <= 128
        && !base.starts_with('.')
        && base
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
    ok.then(|| base.to_string())
}

/// Checks a parsed `custom_media.json`: array of objects with a valid unique key, slot `1..=30`
/// (unique) and a known base medium.
pub fn validate_media_list(value: &Value) -> Result<Vec<CustomMediaJsonEntry>, String> {
    let Some(items) = value.as_array() else {
        return Err(String::from("custom_media.json must be a JSON array"));
    };
    let builtin = builtin_media_specs();
    let mut keys = HashSet::new();
    let mut slots = HashSet::new();
    let mut entries = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        let entry: CustomMediaJsonEntry =
            serde_json::from_value(item.clone()).map_err(|e| format!("entry {i}: {e}"))?;
        let key = entry.key.trim();
        if !valid_key(key) {
            return Err(format!(
                "entry {i}: invalid key {key:?} (1-32 characters A-Z a-z 0-9 _ - .)"
            ));
        }
        if builtin.iter().any(|m| m.key.eq_ignore_ascii_case(key)) {
            return Err(format!(
                "entry {i}: key {key:?} collides with a built-in medium"
            ));
        }
        if !keys.insert(key.to_ascii_lowercase()) {
            return Err(format!("entry {i}: duplicate key {key:?}"));
        }
        if !(1..=30).contains(&entry.slot) {
            return Err(format!("entry {i}: slot {} not in 1..30", entry.slot));
        }
        if !slots.insert(entry.slot) {
            return Err(format!("entry {i}: duplicate slot {}", entry.slot));
        }
        let base_known = if entry.base_media.trim().is_empty() {
            entry
                .base_sn_id
                .is_some_and(|sn| builtin.iter().any(|m| m.sn_id == sn))
        } else {
            builtin
                .iter()
                .any(|m| m.key.eq_ignore_ascii_case(entry.base_media.trim()))
        };
        if !base_known {
            return Err(format!(
                "entry {i} ({key}): unknown base medium {:?}",
                entry.base_media
            ));
        }
        entries.push(entry);
    }
    Ok(entries)
}

fn write_atomic(path: &Path, data: &[u8]) -> Result<(), String> {
    let err = |e: std::io::Error| format!("{}: {e}", path.display());
    let dir = path
        .parent()
        .ok_or_else(|| format!("{}: no parent", path.display()))?;
    fs::create_dir_all(dir).map_err(err)?;
    let tmp = path.with_extension("tmp-install");
    {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o644)
            .open(&tmp)
            .map_err(err)?;
        f.write_all(data).map_err(err)?;
        f.sync_all().map_err(err)?;
    }
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o644)).map_err(err)?;
    fs::rename(&tmp, path).map_err(err)
}

/// Installs the media list of `staging_dir` (`custom_media.json` + ICC files in `icc/`) as the
/// complete system media database. Returns a human-readable report.
///
/// For each entry, `icc_profile` is resolved to (in this order) a profile in `staging_dir/icc/`
/// (copied to the system ICC directory, entry rewritten to its absolute path), an already installed
/// custom profile, or a built-in Epson profile name. Custom profiles no longer referenced are removed.
pub fn install_staged_media(
    staging_dir: &Path,
    paths: &SystemPaths,
) -> Result<Vec<String>, String> {
    let raw = read_untrusted_file(&staging_dir.join("custom_media.json"), MAX_JSON_BYTES)?;
    let mut value: Value =
        serde_json::from_slice(&raw).map_err(|e| format!("custom_media.json: {e}"))?;
    let entries = validate_media_list(&value)?;

    let mut report = Vec::new();
    let mut referenced = HashSet::new();
    let items = value
        .as_array_mut()
        .ok_or("custom_media.json must be an array")?;
    for (item, entry) in items.iter_mut().zip(&entries) {
        let requested = entry.icc_profile.trim();
        if requested.is_empty() {
            continue;
        }
        let Some(name) = safe_icc_name(requested) else {
            return Err(format!(
                "{}: invalid ICC file name {requested:?}",
                entry.key
            ));
        };
        let staged = staging_dir.join("icc").join(&name);
        let installed = paths.icc_dir.join(&name);
        let resolved = if staged.exists() {
            let data = read_untrusted_file(&staged, MAX_ICC_BYTES)?;
            if !is_icc_profile(&data) {
                return Err(format!("{}: not an ICC profile", staged.display()));
            }
            write_atomic(&installed, &data)?;
            report.push(format!("ICC profile installed: {}", installed.display()));
            Some(installed)
        } else if installed.is_file() {
            Some(installed)
        } else if paths.builtin_icc_dir.join(&name).is_file() {
            None // built-in Epson profile, found by name
        } else {
            return Err(format!(
                "{}: ICC profile {name:?} is neither staged nor installed",
                entry.key
            ));
        };
        if let Some(path) = resolved {
            referenced.insert(name.clone());
            item["icc_profile"] = Value::String(path.display().to_string());
        } else {
            item["icc_profile"] = Value::String(name);
        }
    }

    let mut json = serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?;
    json.push(b'\n');
    write_atomic(&paths.media_json, &json)?;
    report.push(format!(
        "{} paper(s) written to {}",
        entries.len(),
        paths.media_json.display()
    ));

    if let Ok(dir) = fs::read_dir(&paths.icc_dir) {
        for file in dir.flatten() {
            let name = file.file_name().to_string_lossy().into_owned();
            let is_profile = safe_icc_name(&name).is_some();
            if is_profile && !referenced.contains(&name) && fs::remove_file(file.path()).is_ok() {
                report.push(format!("Unused ICC profile removed: {name}"));
            }
        }
    }
    Ok(report)
}

/// Removes one medium (by key) from the system database; its custom ICC profile is deleted if no
/// other entry uses it.
pub fn remove_media(key: &str, paths: &SystemPaths) -> Result<Vec<String>, String> {
    let raw =
        fs::read(&paths.media_json).map_err(|e| format!("{}: {e}", paths.media_json.display()))?;
    let mut value: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    let items = value
        .as_array_mut()
        .ok_or("custom_media.json must be an array")?;
    let before = items.len();
    let mut removed_icc = Vec::new();
    items.retain(|item| {
        let matches = item["key"]
            .as_str()
            .is_some_and(|k| k.eq_ignore_ascii_case(key));
        if matches && let Some(icc) = item["icc_profile"].as_str() {
            removed_icc.push(icc.to_string());
        }
        !matches
    });
    if items.len() == before {
        return Err(format!("medium {key:?} is not installed"));
    }
    let still_used: HashSet<String> = items
        .iter()
        .filter_map(|i| i["icc_profile"].as_str().map(String::from))
        .collect();
    let mut json = serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?;
    json.push(b'\n');
    write_atomic(&paths.media_json, &json)?;
    let mut report = vec![format!("Paper {key} removed")];
    for icc in removed_icc {
        let p = PathBuf::from(&icc);
        if !still_used.contains(&icc)
            && p.starts_with(&paths.icc_dir)
            && fs::remove_file(&p).is_ok()
        {
            report.push(format!("ICC profile removed: {icc}"));
        }
    }
    Ok(report)
}

/// CUPS queues whose PPD uses this driver's filter, as `(queue, ppd_path)`.
pub fn driver_queues() -> Vec<(String, PathBuf)> {
    let Ok(out) = Command::new("lpstat").arg("-v").env("LC_ALL", "C").output() else {
        return Vec::new();
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.strip_prefix("device for "))
        .filter_map(|l| l.split_once(':').map(|(q, _)| q.trim().to_string()))
        .filter_map(|q| {
            let ppd = PathBuf::from(format!("/etc/cups/ppd/{q}.ppd"));
            let text = fs::read_to_string(&ppd).ok()?;
            text.contains("rastertop900escpr").then_some((q, ppd))
        })
        .collect()
}

/// Regenerates the PPD of every driver queue from `media_json` (keeping the queue's defaults) and
/// applies it with `lpadmin -P`.
pub fn refresh_queue_ppds(media_json: &Path) -> Result<Vec<String>, String> {
    let registry = MediaRegistry::from_json_paths(&[
        media_json.to_path_buf(),
        PathBuf::from(PACKAGED_CUSTOM_MEDIA_JSON),
    ]);
    let mut report = Vec::new();
    let mut failures = Vec::new();
    for (queue, ppd_path) in driver_queues() {
        let old = fs::read_to_string(&ppd_path).unwrap_or_default();
        let is_p900 = !old.contains("SC-P700");
        let model = if is_p900 {
            "SureColor SC-P900"
        } else {
            "SureColor SC-P700"
        };
        let new_ppd = merge_ppd_defaults(&build_ppd_with(&registry, model, is_p900), &old);
        let tmp =
            std::env::temp_dir().join(format!("epson-p900-{queue}-{}.ppd", std::process::id()));
        if let Err(e) = fs::write(&tmp, new_ppd) {
            failures.push(format!("{queue}: {e}"));
            continue;
        }
        let status = Command::new("lpadmin")
            .args(["-p", &queue, "-P"])
            .arg(&tmp)
            .status();
        let _ = fs::remove_file(&tmp);
        match status {
            Ok(s) if s.success() => report.push(format!("Queue {queue}: PPD updated")),
            Ok(s) => failures.push(format!("{queue}: lpadmin exit {s}")),
            Err(e) => failures.push(format!("{queue}: lpadmin: {e}")),
        }
    }
    if report.is_empty() && failures.is_empty() {
        report.push(String::from("No queue of this driver found"));
    }
    if failures.is_empty() {
        Ok(report)
    } else {
        Err(failures.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn icc_bytes() -> Vec<u8> {
        let mut v = vec![0u8; 200];
        v[36..40].copy_from_slice(b"acsp");
        v
    }

    fn staging(entries: &str, icc: &[(&str, Vec<u8>)]) -> PathBuf {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("epson-p900-staging-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("icc")).unwrap();
        fs::write(dir.join("custom_media.json"), entries).unwrap();
        for (name, data) in icc {
            fs::write(dir.join("icc").join(name), data).unwrap();
        }
        dir
    }

    #[test]
    fn validation_rejects_bad_entries() {
        let ok = serde_json::json!([{"slot": 1, "key": "Slot01_A", "name": "A", "base_media": "PremiumLuster"}]);
        assert!(validate_media_list(&ok).is_ok());
        for bad in [
            serde_json::json!({"not": "an array"}),
            serde_json::json!([{"slot": 1, "key": "bad key!", "base_media": "PremiumLuster"}]),
            serde_json::json!([{"slot": 31, "key": "S", "base_media": "PremiumLuster"}]),
            serde_json::json!([{"slot": 1, "key": "S", "base_media": "NoSuchPaper"}]),
            serde_json::json!([{"slot": 1, "key": "PhotoPaperGlossy", "base_media": "PremiumLuster"}]),
            serde_json::json!([
                {"slot": 1, "key": "A", "base_media": "PremiumLuster"},
                {"slot": 1, "key": "B", "base_media": "PremiumLuster"}
            ]),
        ] {
            assert!(validate_media_list(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn installs_profiles_rewrites_paths_and_prunes() {
        let root = std::env::temp_dir().join(format!("epson-p900-root-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let paths = SystemPaths::under_root(&root);
        fs::create_dir_all(&paths.builtin_icc_dir).unwrap();
        fs::write(paths.builtin_icc_dir.join("Epson_Builtin.icc"), icc_bytes()).unwrap();

        let staged = staging(
            r#"[{"slot": 1, "key": "Slot01_Rag", "name": "Rag", "base_media": "VelvetFineArt", "icc_profile": "Rag.icc", "platen_gap": 9},
                {"slot": 2, "key": "Slot02_Luster", "name": "Luster", "base_media": "PremiumLuster", "icc_profile": "Epson_Builtin.icc"}]"#,
            &[("Rag.icc", icc_bytes()), ("Unused.icc", icc_bytes())],
        );
        fs::create_dir_all(&paths.icc_dir).unwrap();
        fs::write(paths.icc_dir.join("Stale.icc"), icc_bytes()).unwrap();

        install_staged_media(&staged, &paths).unwrap();
        let written: Value = serde_json::from_slice(&fs::read(&paths.media_json).unwrap()).unwrap();
        let rag = paths.icc_dir.join("Rag.icc");
        assert_eq!(written[0]["icc_profile"], rag.display().to_string());
        assert_eq!(written[0]["platen_gap"], 9, "unknown fields are kept");
        assert_eq!(written[1]["icc_profile"], "Epson_Builtin.icc");
        assert!(rag.is_file());
        assert!(!paths.icc_dir.join("Stale.icc").exists());
        assert!(!paths.icc_dir.join("Unused.icc").exists());

        // The filter's registry resolves the installed medium.
        let reg = MediaRegistry::from_json_paths(std::slice::from_ref(&paths.media_json));
        let rag_spec = reg.find("Slot01_Rag").unwrap();
        assert_eq!(rag_spec.icc_profile, rag.display().to_string());

        remove_media("Slot01_Rag", &paths).unwrap();
        assert!(!rag.exists());
        assert!(
            MediaRegistry::from_json_paths(std::slice::from_ref(&paths.media_json))
                .find("Slot01_Rag")
                .is_none()
        );
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&staged);
    }

    #[test]
    fn rejects_symlinks_fake_profiles_and_missing_profiles() {
        let root = std::env::temp_dir().join(format!("epson-p900-root2-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let paths = SystemPaths::under_root(&root);
        let entry =
            r#"[{"slot": 1, "key": "S1", "base_media": "PremiumLuster", "icc_profile": "x.icc"}]"#;

        let fake = staging(entry, &[("x.icc", b"not a profile".to_vec())]);
        assert!(install_staged_media(&fake, &paths).is_err());

        let missing = staging(entry, &[]);
        assert!(install_staged_media(&missing, &paths).is_err());

        let linked = staging(entry, &[]);
        std::os::unix::fs::symlink("/etc/hostname", linked.join("icc").join("x.icc")).unwrap();
        assert!(install_staged_media(&linked, &paths).is_err());
        assert!(!paths.media_json.exists(), "nothing is written on error");
        for d in [fake, missing, linked, root] {
            let _ = fs::remove_dir_all(d);
        }
    }
}
