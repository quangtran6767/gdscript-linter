//! Owner: Track A (W4). Lint cache in `.gdguard/cache/lint.json`, keyed by
//! file hash, config hash, rules hash and tool version.
//!
//! The cache lives next to the project's `gdguard.toml`; without a config
//! file nothing is cached. An entry is reused when the file content, the
//! resolved settings for that file (levels, options, line length, indent
//! size) and the custom rule files are unchanged; a new tool version drops
//! the whole cache. The folder gets a `.gitignore` so it's never committed.
//!
//! Writes go to a temporary file that is then renamed, so parallel runs
//! never read a half-written cache. A cache that can't be read is ignored.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::linter::api::Config;
use crate::linter::config::FileSettings;
use crate::linter::custom::loader::find_rule_files;
use crate::linter::diagnostic::{Diagnostic, Edit, Fix, FixSafety, Severity, Span};

// The W10 review changes fix safety and adds source-slice fixes. Drop older
// results so cached safe fixes cannot bypass the corrected dependency check.
pub const CACHE_VERSION: u32 = 6;

/// The cache file, relative to the folder of `gdguard.toml`.
pub const CACHE_PATH: &str = ".gdguard/cache/lint.json";

/// FNV-1a 64-bit hash of several byte strings, each followed by a 0xff
/// separator. Stable across platforms and runs, unlike `std`'s hasher.
pub fn stable_hash(parts: &[&[u8]]) -> String {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET_BASIS;
    for part in parts {
        for byte in *part {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(PRIME);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(PRIME);
    }
    format!("{:016x}", hash)
}

/// Hash of a file's content.
pub fn file_hash(source: &str) -> String {
    stable_hash(&[source.as_bytes()])
}

/// Hash of everything in `settings` that changes a file's diagnostics.
pub fn settings_hash(settings: &FileSettings) -> String {
    let mut disabled_rules: Vec<&String> = Vec::new();
    for rule in &settings.linter_config.disabled_rules {
        disabled_rules.push(rule);
    }
    disabled_rules.sort();
    // `levels` and `rule_options` are BTreeMaps, so their Debug text is
    // ordered. The tool version is part of the cache key, so a change in the
    // Debug format between versions can't produce a false hit.
    let text = format!(
        "disabled={:?};max_line_length={};indent_size={};levels={:?};options={:?}",
        disabled_rules,
        settings.linter_config.max_line_length,
        settings.linter_config.indent_size,
        settings.levels,
        settings.rule_options
    );
    stable_hash(&[text.as_bytes()])
}

/// Hash of the custom rule files of `config` (their paths and contents).
/// Folders that can't be read hash as empty: loading the rules reports
/// that error before the cache is used.
pub fn rules_hash(config: &Config) -> String {
    let mut parts: Vec<Vec<u8>> = Vec::new();
    for folder in &config.custom_rule_paths {
        let Ok(mut files) = find_rule_files(folder) else {
            continue;
        };
        if let Ok(js_files) = crate::linter::custom::find_js_files(std::slice::from_ref(folder)) {
            files.extend(js_files);
            files.sort();
        }
        for file in files {
            parts.push(file.to_string_lossy().into_owned().into_bytes());
            match fs::read(&file) {
                Ok(content) => parts.push(content),
                Err(_) => parts.push(Vec::new()),
            }
        }
    }
    let mut slices: Vec<&[u8]> = Vec::with_capacity(parts.len());
    for part in &parts {
        slices.push(part);
    }
    stable_hash(&slices)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CachedEdit {
    start_byte: usize,
    end_byte: usize,
    replacement: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CachedFix {
    description: String,
    safety: String,
    edits: Vec<CachedEdit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CachedDiagnostic {
    rule: String,
    severity: String,
    message: String,
    help: Option<String>,
    start_byte: usize,
    end_byte: usize,
    fixes: Vec<CachedFix>,
}

/// The cached result for one file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheEntry {
    file_hash: String,
    settings_hash: String,
    rules_hash: String,
    diagnostics: Vec<CachedDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CacheFile {
    version: u32,
    tool_version: String,
    /// Keyed by absolute file path.
    files: BTreeMap<String, CacheEntry>,
}

/// What identifies one file's lint result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheKey {
    pub file_hash: String,
    pub settings_hash: String,
    pub rules_hash: String,
}

/// The cache of one run. Read-only while files are linted (it's shared by
/// the worker threads); new entries are added afterwards.
#[derive(Debug, Clone, Default)]
pub struct LintCache {
    path: PathBuf,
    entries: BTreeMap<String, CacheEntry>,
    is_changed: bool,
}

/// The key under which a file is stored: its absolute path with `/`.
pub fn file_key(path: &Path) -> String {
    let absolute = match std::path::absolute(path) {
        Ok(absolute) => absolute,
        Err(_) => path.to_path_buf(),
    };
    absolute.to_string_lossy().replace('\\', "/")
}

impl LintCache {
    /// Loads the cache at `path`. A missing, unreadable or outdated cache
    /// gives an empty one.
    pub fn load(path: &Path) -> LintCache {
        let mut cache = LintCache {
            path: path.to_path_buf(),
            entries: BTreeMap::new(),
            is_changed: false,
        };
        let Ok(text) = fs::read_to_string(path) else {
            return cache;
        };
        let Ok(file) = serde_json::from_str::<CacheFile>(&text) else {
            return cache;
        };
        if file.version == CACHE_VERSION && file.tool_version == env!("CARGO_PKG_VERSION") {
            cache.entries = file.files;
        }
        cache
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The cached diagnostics of `file_key` when `key` matches, rebuilt
    /// against `source` (which has the cached hash).
    pub fn get(&self, file_key: &str, key: &CacheKey, source: &str) -> Option<Vec<Diagnostic>> {
        let entry = self.entries.get(file_key)?;
        if entry.file_hash != key.file_hash
            || entry.settings_hash != key.settings_hash
            || entry.rules_hash != key.rules_hash
        {
            return None;
        }
        let mut diagnostics = Vec::with_capacity(entry.diagnostics.len());
        for cached in &entry.diagnostics {
            diagnostics.push(restore_diagnostic(cached, source)?);
        }
        Some(diagnostics)
    }

    pub fn insert(&mut self, file_key: String, key: CacheKey, diagnostics: &[Diagnostic]) {
        let mut cached = Vec::with_capacity(diagnostics.len());
        for diagnostic in diagnostics {
            cached.push(store_diagnostic(diagnostic));
        }
        self.entries.insert(
            file_key,
            CacheEntry {
                file_hash: key.file_hash,
                settings_hash: key.settings_hash,
                rules_hash: key.rules_hash,
                diagnostics: cached,
            },
        );
        self.is_changed = true;
    }

    /// Writes the cache when entries were added. Drops entries of files that
    /// no longer exist. Creates the folder and its `.gitignore`.
    pub fn save(&mut self) -> Result<(), String> {
        if !self.is_changed {
            return Ok(());
        }
        let mut stale_keys = Vec::new();
        for file_key in self.entries.keys() {
            if !Path::new(file_key).exists() {
                stale_keys.push(file_key.clone());
            }
        }
        for file_key in stale_keys {
            self.entries.remove(&file_key);
        }

        let Some(folder) = self.path.parent() else {
            return Err(format!("Invalid cache path {}", self.path.display()));
        };
        fs::create_dir_all(folder)
            .map_err(|error| format!("Failed to create {}: {}", folder.display(), error))?;
        let gitignore = folder.join(".gitignore");
        if !gitignore.exists() {
            fs::write(&gitignore, "# Created by gdguard.\n*\n")
                .map_err(|error| format!("Failed to write {}: {}", gitignore.display(), error))?;
        }
        let file = CacheFile {
            version: CACHE_VERSION,
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            files: self.entries.clone(),
        };
        let text = serde_json::to_string(&file)
            .map_err(|error| format!("Failed to serialize the lint cache: {}", error))?;
        let nanos = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            Ok(duration) => duration.as_nanos(),
            Err(_) => 0,
        };
        let temporary = folder.join(format!("lint.json.{}-{}.tmp", std::process::id(), nanos));
        fs::write(&temporary, text)
            .map_err(|error| format!("Failed to write {}: {}", temporary.display(), error))?;
        fs::rename(&temporary, &self.path).map_err(|error| {
            let _ = fs::remove_file(&temporary);
            format!("Failed to write {}: {}", self.path.display(), error)
        })?;
        self.is_changed = false;
        Ok(())
    }
}

fn store_diagnostic(diagnostic: &Diagnostic) -> CachedDiagnostic {
    let mut fixes = Vec::with_capacity(diagnostic.fixes.len());
    for fix in &diagnostic.fixes {
        let mut edits = Vec::with_capacity(fix.edits.len());
        for edit in &fix.edits {
            edits.push(CachedEdit {
                start_byte: edit.span.start_byte,
                end_byte: edit.span.end_byte,
                replacement: edit.replacement.clone(),
            });
        }
        fixes.push(CachedFix {
            description: fix.description.clone(),
            safety: fix.safety.as_str().to_string(),
            edits,
        });
    }
    CachedDiagnostic {
        rule: diagnostic.rule.clone(),
        severity: diagnostic.severity.as_str().to_string(),
        message: diagnostic.message.clone(),
        help: diagnostic.help.clone(),
        start_byte: diagnostic.span.start_byte,
        end_byte: diagnostic.span.end_byte,
        fixes,
    }
}

/// Rebuilds a diagnostic; None when the cached data doesn't fit `source`.
fn restore_diagnostic(cached: &CachedDiagnostic, source: &str) -> Option<Diagnostic> {
    let span = restore_span(source, cached.start_byte, cached.end_byte)?;
    let mut fixes = Vec::with_capacity(cached.fixes.len());
    for cached_fix in &cached.fixes {
        let mut edits = Vec::with_capacity(cached_fix.edits.len());
        for cached_edit in &cached_fix.edits {
            edits.push(Edit {
                span: restore_span(source, cached_edit.start_byte, cached_edit.end_byte)?,
                replacement: cached_edit.replacement.clone(),
            });
        }
        fixes.push(Fix {
            description: cached_fix.description.clone(),
            safety: FixSafety::from_name(&cached_fix.safety)?,
            edits,
        });
    }
    Some(Diagnostic {
        rule: cached.rule.clone(),
        severity: Severity::from_name(&cached.severity)?,
        message: cached.message.clone(),
        help: cached.help.clone(),
        span,
        fixes,
    })
}

fn restore_span(source: &str, start_byte: usize, end_byte: usize) -> Option<Span> {
    if start_byte > end_byte
        || end_byte > source.len()
        || !source.is_char_boundary(start_byte)
        || !source.is_char_boundary(end_byte)
    {
        return None;
    }
    Some(Span::from_byte_range(source, start_byte, end_byte))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn sample_diagnostic(source: &str) -> Diagnostic {
        let start = source.find("pass").unwrap();
        let span = Span::from_byte_range(source, start, start + 4);
        Diagnostic {
            rule: "unnecessary-pass".to_string(),
            severity: Severity::Warning,
            message: "Unnecessary 'pass'".to_string(),
            help: Some("Remove it.".to_string()),
            span,
            fixes: vec![Fix {
                description: "Remove the 'pass' line".to_string(),
                safety: FixSafety::Safe,
                edits: vec![Edit {
                    span,
                    replacement: String::new(),
                }],
            }],
        }
    }

    fn key(source: &str) -> CacheKey {
        CacheKey {
            file_hash: file_hash(source),
            settings_hash: "settings".to_string(),
            rules_hash: "rules".to_string(),
        }
    }

    #[test]
    fn round_trips_diagnostics_and_misses_on_any_key_change() {
        let folder =
            std::env::temp_dir().join(format!("gdguard-cache-unit-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        let source_path = folder.join("a.gd");
        fs::create_dir_all(&folder).unwrap();
        let source = "func f():\n\tprint(1)\n\tpass\n";
        fs::write(&source_path, source).unwrap();
        let cache_path = folder.join(CACHE_PATH);

        let mut cache = LintCache::load(&cache_path);
        assert!(cache.is_empty());
        let diagnostic = sample_diagnostic(source);
        cache.insert(
            file_key(&source_path),
            key(source),
            std::slice::from_ref(&diagnostic),
        );
        cache.save().unwrap();
        assert_eq!(
            fs::read_to_string(folder.join(".gdguard/cache/.gitignore")).unwrap(),
            "# Created by gdguard.\n*\n"
        );

        let loaded = LintCache::load(&cache_path);
        assert_eq!(loaded.len(), 1);
        let file = file_key(&source_path);
        assert_eq!(
            loaded.get(&file, &key(source), source),
            Some(vec![diagnostic])
        );

        let edited = "func f():\n\tprint(2)\n\tpass\n";
        assert_eq!(loaded.get(&file, &key(edited), edited), None);
        let mut other_settings = key(source);
        other_settings.settings_hash = "other".to_string();
        assert_eq!(loaded.get(&file, &other_settings, source), None);
        let mut other_rules = key(source);
        other_rules.rules_hash = "other".to_string();
        assert_eq!(loaded.get(&file, &other_rules, source), None);

        fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn ignores_unreadable_or_outdated_caches() {
        let folder = std::env::temp_dir().join(format!("gdguard-cache-bad-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir_all(&folder).unwrap();
        let cache_path = folder.join("lint.json");
        fs::write(&cache_path, "not json").unwrap();
        assert!(LintCache::load(&cache_path).is_empty());
        fs::write(
            &cache_path,
            "{\"version\":1,\"tool_version\":\"0.0.0-old\",\"files\":{\"/a.gd\":{\"file_hash\":\"x\",\"settings_hash\":\"y\",\"rules_hash\":\"z\",\"diagnostics\":[]}}}",
        )
        .unwrap();
        assert!(LintCache::load(&cache_path).is_empty());
        fs::remove_dir_all(&folder).unwrap();
    }
}
