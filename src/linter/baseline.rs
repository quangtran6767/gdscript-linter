//! Owner: Track A (W4). Baseline file: `--update-baseline` writes every
//! current issue, `--baseline` (or `[lint].baseline`) hides them.
//!
//! An issue's fingerprint is a hash of its rule, its file (relative to the
//! baseline root, with `/`), the trimmed text of the line it starts on, and
//! the occurrence index of that (rule, line text) pair in the file. Issues
//! survive edits that move lines; a new copy of a baselined line is a new
//! occurrence and is reported.
//!
//! The file is JSON, sorted by file, rule and fingerprint, so updates give
//! small diffs:
//!
//! ```json
//! {
//!   "version": 1,
//!   "issues": [
//!     { "file": "scripts/player.gd", "rule": "unused-argument",
//!       "fingerprint": "5c1f0e2a9b3d4c7e", "message": "..." }
//!   ]
//! }
//! ```

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::linter::config;
use crate::linter::output::{FileDiagnostics, LintReport, line_text_at};

pub const BASELINE_VERSION: u32 = 1;

/// Where `--update-baseline` writes when neither `--baseline` nor
/// `[lint].baseline` names a file, relative to the baseline root.
pub const DEFAULT_BASELINE_PATH: &str = ".gdguard/baseline.json";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BaselineEntry {
    pub file: String,
    pub rule: String,
    /// 16 hex digits.
    pub fingerprint: String,
    /// For people reading the file. Not part of the fingerprint.
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct BaselineFile {
    version: u32,
    issues: Vec<BaselineEntry>,
}

/// A loaded baseline: how many times each (file, rule, fingerprint) may be
/// hidden.
#[derive(Debug, Clone, Default)]
pub struct Baseline {
    counts: HashMap<(String, String, String), usize>,
}

impl Baseline {
    pub fn from_entries(entries: &[BaselineEntry]) -> Baseline {
        let mut counts = HashMap::with_capacity(entries.len());
        for entry in entries {
            let key = (
                entry.file.clone(),
                entry.rule.clone(),
                entry.fingerprint.clone(),
            );
            *counts.entry(key).or_insert(0) += 1;
        }
        Baseline { counts }
    }

    pub fn len(&self) -> usize {
        let mut total = 0;
        for count in self.counts.values() {
            total += count;
        }
        total
    }

    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }
}

/// Where the baseline of a run lives and what its paths are relative to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineTarget {
    /// The baseline file.
    pub path: PathBuf,
    /// The folder baseline file paths are relative to: the folder of the
    /// config found from the current folder (or `--config`), else the
    /// current folder.
    pub root: PathBuf,
    /// True when the path comes from `--baseline`. A missing explicit file is
    /// an error; a missing `[lint].baseline` file hides nothing.
    pub is_explicit: bool,
}

/// Resolves the baseline for a run: `--baseline` (relative to the current
/// folder), else `[lint].baseline` from the config. With `for_update`, falls
/// back to `.gdguard/baseline.json` in the root; otherwise returns None when
/// no baseline is configured.
pub fn resolve_baseline(
    cli_path: Option<&Path>,
    config_path: Option<&Path>,
    current_directory: &Path,
    for_update: bool,
) -> Result<Option<BaselineTarget>, String> {
    let config_file = match config_path {
        Some(config_path) => {
            let config_file = current_directory.join(config_path);
            if !config_file.is_file() {
                return Err(format!("--config: {} is not a file", config_path.display()));
            }
            Some(config_file)
        }
        None => config::find_config_file(current_directory),
    };
    let (root, config_baseline) = match &config_file {
        Some(config_file) => {
            let config =
                config::load_config_file(config_file).map_err(|error| error.to_string())?;
            let root = match config_file.parent() {
                Some(parent) => parent.to_path_buf(),
                None => current_directory.to_path_buf(),
            };
            (root, config.baseline)
        }
        None => (current_directory.to_path_buf(), None),
    };
    if let Some(cli_path) = cli_path {
        return Ok(Some(BaselineTarget {
            path: current_directory.join(cli_path),
            root,
            is_explicit: true,
        }));
    }
    if let Some(config_baseline) = config_baseline {
        return Ok(Some(BaselineTarget {
            path: config_baseline,
            root,
            is_explicit: false,
        }));
    }
    if for_update {
        return Ok(Some(BaselineTarget {
            path: root.join(DEFAULT_BASELINE_PATH),
            root,
            is_explicit: false,
        }));
    }
    Ok(None)
}

/// Loads the baseline of `target`. A missing file that came from
/// `[lint].baseline` gives an empty baseline.
pub fn load_target(target: &BaselineTarget) -> Result<Baseline, String> {
    if !target.is_explicit && !target.path.exists() {
        return Ok(Baseline::default());
    }
    load_baseline(&target.path)
}

/// Reads a baseline file. Errors name the file.
pub fn load_baseline(path: &Path) -> Result<Baseline, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read baseline {}: {}", path.display(), error))?;
    let file: BaselineFile = serde_json::from_str(&text)
        .map_err(|error| format!("Invalid baseline {}: {}", path.display(), error))?;
    if file.version != BASELINE_VERSION {
        return Err(format!(
            "Invalid baseline {}: unsupported version {} (expected {})",
            path.display(),
            file.version,
            BASELINE_VERSION
        ));
    }
    Ok(Baseline::from_entries(&file.issues))
}

/// Writes `entries` sorted, as pretty JSON with a final newline. Creates the
/// parent folder.
pub fn write_baseline(path: &Path, mut entries: Vec<BaselineEntry>) -> Result<(), String> {
    entries.sort();
    let file = BaselineFile {
        version: BASELINE_VERSION,
        issues: entries,
    };
    let mut text = serde_json::to_string_pretty(&file)
        .map_err(|error| format!("Failed to serialize baseline: {}", error))?;
    text.push('\n');
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create {}: {}", parent.display(), error))?;
    }
    fs::write(path, text)
        .map_err(|error| format!("Failed to write baseline {}: {}", path.display(), error))
}

/// Builds baseline entries for every issue in `report`. `base_directory` is
/// the folder report paths are relative to (the current folder);
/// `baseline_root` is the folder baseline paths are relative to.
pub fn entries_for_report(
    report: &LintReport,
    base_directory: &Path,
    baseline_root: &Path,
) -> Vec<BaselineEntry> {
    let mut entries = Vec::new();
    for file in &report.files {
        let file_key = baseline_file_key(&file.path, base_directory, baseline_root);
        let fingerprints = fingerprints_for_file(&file_key, file);
        for (index, diagnostic) in file.diagnostics.iter().enumerate() {
            entries.push(BaselineEntry {
                file: file_key.clone(),
                rule: diagnostic.rule.clone(),
                fingerprint: fingerprints[index].clone(),
                message: diagnostic.message.clone(),
            });
        }
    }
    entries
}

/// Removes the issues listed in `baseline` from `report` and returns how
/// many were hidden. Each baseline entry hides at most one issue. Files left
/// without issues are removed from `report.files`.
pub fn apply_baseline(
    baseline: &Baseline,
    report: &mut LintReport,
    base_directory: &Path,
    baseline_root: &Path,
) -> usize {
    if baseline.is_empty() {
        return 0;
    }
    let mut remaining_counts = baseline.counts.clone();
    let mut hidden = 0;
    let mut kept_files = Vec::with_capacity(report.files.len());
    for mut file in report.files.drain(..) {
        let file_key = baseline_file_key(&file.path, base_directory, baseline_root);
        let fingerprints = fingerprints_for_file(&file_key, &file);
        let mut kept = Vec::with_capacity(file.diagnostics.len());
        for (index, diagnostic) in file.diagnostics.drain(..).enumerate() {
            let key = (
                file_key.clone(),
                diagnostic.rule.clone(),
                fingerprints[index].clone(),
            );
            match remaining_counts.get_mut(&key) {
                Some(count) if *count > 0 => {
                    *count -= 1;
                    hidden += 1;
                }
                _ => kept.push(diagnostic),
            }
        }
        if !kept.is_empty() {
            file.diagnostics = kept;
            kept_files.push(file);
        }
    }
    report.files = kept_files;
    hidden
}

/// The fingerprint of each diagnostic of `file`, in order. The occurrence
/// index counts earlier diagnostics (in position order) with the same rule
/// and the same trimmed line text.
fn fingerprints_for_file(file_key: &str, file: &FileDiagnostics) -> Vec<String> {
    let mut occurrences: HashMap<(String, String), usize> = HashMap::new();
    let mut fingerprints = Vec::with_capacity(file.diagnostics.len());
    for diagnostic in &file.diagnostics {
        let line_text = line_text_at(&file.source, diagnostic.span.start_byte)
            .trim()
            .to_string();
        let occurrence_key = (diagnostic.rule.clone(), line_text.clone());
        let occurrence = occurrences.entry(occurrence_key).or_insert(0);
        fingerprints.push(fingerprint(
            &diagnostic.rule,
            file_key,
            &line_text,
            *occurrence,
        ));
        *occurrence += 1;
    }
    fingerprints
}

/// FNV-1a 64-bit over the fields, each followed by a 0xff byte (never in
/// UTF-8 text). Stable across
/// platforms and versions, unlike `std`'s hasher.
pub fn fingerprint(rule: &str, file: &str, line_text: &str, occurrence: usize) -> String {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET_BASIS;
    let occurrence_text = occurrence.to_string();
    let fields = [rule, file, line_text, occurrence_text.as_str()];
    for field in fields {
        for byte in field.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(PRIME);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(PRIME);
    }
    format!("{:016x}", hash)
}

/// The file path stored in the baseline: relative to `baseline_root` with
/// `/` separators when the file is inside it, else the display path.
pub fn baseline_file_key(
    display_path: &str,
    base_directory: &Path,
    baseline_root: &Path,
) -> String {
    let path = Path::new(display_path);
    let absolute: PathBuf = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_directory.join(path)
    };
    let relative = match absolute.strip_prefix(baseline_root) {
        Ok(relative) => relative,
        Err(_) => path,
    };
    let mut key = String::new();
    for component in relative.components() {
        let text = component.as_os_str().to_string_lossy();
        if text == "." {
            continue;
        }
        if !key.is_empty() {
            key.push('/');
        }
        key.push_str(&text);
    }
    key
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn fingerprint_is_stable_and_depends_on_every_field() {
        let base = fingerprint("rule", "a.gd", "var x = 1", 0);
        assert_eq!(base, fingerprint("rule", "a.gd", "var x = 1", 0));
        assert_eq!(base.len(), 16);
        assert_ne!(base, fingerprint("other", "a.gd", "var x = 1", 0));
        assert_ne!(base, fingerprint("rule", "b.gd", "var x = 1", 0));
        assert_ne!(base, fingerprint("rule", "a.gd", "var x = 2", 0));
        assert_ne!(base, fingerprint("rule", "a.gd", "var x = 1", 1));
        // Field boundaries matter.
        assert_ne!(fingerprint("ab", "c", "", 0), fingerprint("a", "bc", "", 0));
    }

    #[test]
    fn file_keys_are_relative_to_the_root_with_slashes() {
        let root = Path::new("/project");
        assert_eq!(
            baseline_file_key("scripts/a.gd", Path::new("/project"), root),
            "scripts/a.gd"
        );
        assert_eq!(
            baseline_file_key("a.gd", Path::new("/project/scripts"), root),
            "scripts/a.gd"
        );
        assert_eq!(
            baseline_file_key("/project/scripts/a.gd", Path::new("/elsewhere"), root),
            "scripts/a.gd"
        );
        assert_eq!(
            baseline_file_key("../other/a.gd", Path::new("/project"), root),
            "../other/a.gd"
        );
    }
}
