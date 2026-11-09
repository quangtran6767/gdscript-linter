//! Owner: Track A (W3). Fix engine: multi-pass `--fix`/`--fix-unsafe` with
//! overlap handling, re-parse and rollback, and the optional formatter step.
//!
//! One file goes through these steps:
//!
//! 1. Lint, then collect the first fix of each diagnostic that the safety
//!    level allows (`Safe`, plus `Unsafe` with `--fix-unsafe`).
//! 2. Sort the fixes by position and keep only those that don't overlap a
//!    fix already chosen. Skipped fixes come back on the next pass.
//! 3. Apply the chosen fixes and re-parse. If the result has syntax errors,
//!    apply the fixes one at a time to find the ones that break the code,
//!    drop them, and report them as `fix-failed` at the end.
//! 4. Re-lint and repeat, for at most `MAX_FIX_PASSES` passes, stopping when
//!    no fix is left.
//! 5. If the input was formatted, format the result.

use std::path::Path;

use crate::linter::config::FileSettings;
use crate::linter::diagnostic::{Diagnostic, Edit, FixSafety, RuleProvider, Severity, apply_edits};
use crate::linter::{GDScriptLinter, apply_rule_levels, lint_with_settings};
use crate::{FormatterConfiguration, format_gdscript};

/// The fix engine stops after this many passes even if fixes are left, so a
/// fix that keeps producing new issues can't loop forever.
pub const MAX_FIX_PASSES: usize = 10;

/// The rule id of the diagnostic reported when a fix would break the code.
pub const FIX_FAILED_RULE_ID: &str = "fix-failed";

/// The result of fixing one buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedSource {
    /// The source after fixing (and formatting, when the input was
    /// formatted). Equal to the input when nothing changed.
    pub output: String,
    /// How many fixes were applied, over all passes.
    pub applied: usize,
    /// Diagnostics left in `output`, plus one `fix-failed` diagnostic for
    /// each issue whose fix was dropped. Sorted by position.
    pub remaining: Vec<Diagnostic>,
}

/// Identifies a fix by content rather than position, so a fix that failed
/// once is recognized on later passes after other fixes moved the code.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FixKey {
    rule: String,
    /// `(replaced text, replacement)` per edit.
    edits: Vec<(String, String)>,
}

/// A fix chosen from a diagnostic, with the byte range it touches.
struct FixCandidate {
    edits: Vec<Edit>,
    start_byte: usize,
    end_byte: usize,
    key: FixKey,
}

/// Fixes one buffer: lints it, applies fixes over several passes and
/// re-lints the result. `allow_unsafe` adds `Unsafe` fixes to `Safe` ones.
///
/// A buffer that already has syntax errors is returned unchanged, with its
/// diagnostics: the engine can't tell whether a fix made it worse. The one
/// exception is a set of fixes that together remove every syntax error (for
/// example Godot 3 keywords): that result is accepted, then fixed as usual.
pub fn fix_source_with_settings(
    linter: &mut GDScriptLinter,
    path: &Path,
    source: &str,
    settings: &FileSettings,
    providers: &mut [Box<dyn RuleProvider>],
    allow_unsafe: bool,
) -> Result<FixedSource, String> {
    let mut diagnostics = lint_with_settings(linter, path, source, settings, providers)?;
    let mut current = source.to_string();
    let mut applied = 0;
    if linter.has_syntax_errors(source)? {
        let mut candidates = Vec::new();
        collect_candidates(&diagnostics, source, allow_unsafe, &[], &mut candidates);
        let chosen = choose_non_overlapping(candidates);
        let mut all_edits: Vec<Edit> = Vec::new();
        for candidate in &chosen {
            all_edits.extend(candidate.edits.iter().cloned());
        }
        let repaired = match apply_edits(source, &all_edits) {
            Ok(fixed) if !chosen.is_empty() && !linter.has_syntax_errors(&fixed)? => Some(fixed),
            _ => None,
        };
        let Some(repaired) = repaired else {
            return Ok(FixedSource {
                output: source.to_string(),
                applied: 0,
                remaining: diagnostics,
            });
        };
        applied = chosen.len();
        current = repaired;
        diagnostics = lint_with_settings(linter, path, &current, settings, providers)?;
    }

    let mut failed_keys: Vec<FixKey> = Vec::new();
    for _pass in 0..MAX_FIX_PASSES {
        let mut candidates = Vec::new();
        collect_candidates(
            &diagnostics,
            &current,
            allow_unsafe,
            &failed_keys,
            &mut candidates,
        );
        if candidates.is_empty() {
            break;
        }
        let chosen = choose_non_overlapping(candidates);
        let (fixed, accepted) = apply_checked(linter, &current, &chosen, &mut failed_keys)?;
        if accepted == 0 {
            // Every chosen fix failed. The next pass skips them and may pick
            // fixes they overlapped.
            continue;
        }
        applied += accepted;
        current = fixed;
        diagnostics = lint_with_settings(linter, path, &current, settings, providers)?;
    }

    if applied > 0 {
        let mut formatter_config = FormatterConfiguration::default();
        crate::editorconfig::apply_editorconfig_to_formatter_config(&mut formatter_config, path);
        if is_formatted(source, &formatter_config)
            && let Ok(formatted) = format_gdscript(&current, &formatter_config)
            && formatted != current
        {
            current = formatted;
            diagnostics = lint_with_settings(linter, path, &current, settings, providers)?;
        }
    }

    if !failed_keys.is_empty() {
        let mut failed_diagnostics = Vec::new();
        for diagnostic in &diagnostics {
            let Some(edits) = first_allowed_fix_edits(diagnostic, allow_unsafe) else {
                continue;
            };
            let key = fix_key(&diagnostic.rule, edits, &current);
            if failed_keys.contains(&key) {
                failed_diagnostics.push(fix_failed_diagnostic(diagnostic));
            }
        }
        apply_rule_levels(&settings.levels, &mut failed_diagnostics);
        diagnostics.extend(failed_diagnostics);
        diagnostics.sort_by(|a, b| {
            a.span
                .start_byte
                .cmp(&b.span.start_byte)
                .then(a.span.end_byte.cmp(&b.span.end_byte))
        });
    }

    Ok(FixedSource {
        output: current,
        applied,
        remaining: diagnostics,
    })
}

/// Returns true when formatting `source` gives the same text. Sources that
/// can't be formatted count as not formatted.
pub fn is_formatted(source: &str, config: &FormatterConfiguration) -> bool {
    match format_gdscript(source, config) {
        Ok(formatted) => formatted == source,
        Err(_) => false,
    }
}

/// Returns the edits of the first fix that `allow_unsafe` permits, or None.
/// `Suggestion` fixes are never applied.
fn first_allowed_fix_edits(diagnostic: &Diagnostic, allow_unsafe: bool) -> Option<&[Edit]> {
    for fix in &diagnostic.fixes {
        let is_allowed = match fix.safety {
            FixSafety::Safe => true,
            FixSafety::Unsafe => allow_unsafe,
            FixSafety::Suggestion => false,
        };
        if is_allowed && !fix.edits.is_empty() {
            return Some(&fix.edits);
        }
    }
    None
}

fn fix_key(rule: &str, edits: &[Edit], source: &str) -> FixKey {
    let mut key_edits = Vec::with_capacity(edits.len());
    for edit in edits {
        let replaced = match source.get(edit.span.start_byte..edit.span.end_byte) {
            Some(text) => text.to_string(),
            None => String::new(),
        };
        key_edits.push((replaced, edit.replacement.clone()));
    }
    FixKey {
        rule: rule.to_string(),
        edits: key_edits,
    }
}

/// Collects one fix per diagnostic, skipping fixes that failed before.
fn collect_candidates(
    diagnostics: &[Diagnostic],
    source: &str,
    allow_unsafe: bool,
    failed_keys: &[FixKey],
    candidates: &mut Vec<FixCandidate>,
) {
    for diagnostic in diagnostics {
        let Some(edits) = first_allowed_fix_edits(diagnostic, allow_unsafe) else {
            continue;
        };
        let key = fix_key(&diagnostic.rule, edits, source);
        if failed_keys.contains(&key) {
            continue;
        }
        let mut start_byte = usize::MAX;
        let mut end_byte = 0;
        for edit in edits {
            start_byte = start_byte.min(edit.span.start_byte);
            end_byte = end_byte.max(edit.span.end_byte);
        }
        candidates.push(FixCandidate {
            edits: edits.to_vec(),
            start_byte,
            end_byte,
            key,
        });
    }
}

/// Sorts the candidates by position and keeps those that don't overlap a
/// fix already kept. Two fixes that start at the same byte count as
/// overlapping, so their order never depends on the rule order.
fn choose_non_overlapping(mut candidates: Vec<FixCandidate>) -> Vec<FixCandidate> {
    candidates.sort_by(|a, b| {
        a.start_byte
            .cmp(&b.start_byte)
            .then(a.end_byte.cmp(&b.end_byte))
    });
    let mut chosen: Vec<FixCandidate> = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        if let Some(previous) = chosen.last()
            && (candidate.start_byte < previous.end_byte
                || candidate.start_byte == previous.start_byte)
        {
            continue;
        }
        chosen.push(candidate);
    }
    chosen
}

/// Applies the chosen fixes and checks that the result still parses. When it
/// doesn't, adds the fixes one at a time and keeps only those that leave the
/// code valid; the others go to `failed_keys`. Returns the new source and how
/// many fixes were applied.
fn apply_checked(
    linter: &mut GDScriptLinter,
    source: &str,
    chosen: &[FixCandidate],
    failed_keys: &mut Vec<FixKey>,
) -> Result<(String, usize), String> {
    let mut all_edits: Vec<Edit> = Vec::new();
    for candidate in chosen {
        all_edits.extend(candidate.edits.iter().cloned());
    }
    if let Ok(fixed) = apply_edits(source, &all_edits)
        && !linter.has_syntax_errors(&fixed)?
    {
        return Ok((fixed, chosen.len()));
    }

    let mut accepted_edits: Vec<Edit> = Vec::new();
    let mut accepted_text = source.to_string();
    let mut accepted = 0;
    for candidate in chosen {
        let mut trial_edits = accepted_edits.clone();
        trial_edits.extend(candidate.edits.iter().cloned());
        let is_valid = match apply_edits(source, &trial_edits) {
            Ok(fixed) => {
                if linter.has_syntax_errors(&fixed)? {
                    false
                } else {
                    accepted_text = fixed;
                    true
                }
            }
            Err(_) => false,
        };
        if is_valid {
            accepted_edits = trial_edits;
            accepted += 1;
        } else {
            failed_keys.push(candidate.key.clone());
        }
    }
    Ok((accepted_text, accepted))
}

fn fix_failed_diagnostic(diagnostic: &Diagnostic) -> Diagnostic {
    Diagnostic {
        rule: FIX_FAILED_RULE_ID.to_string(),
        severity: Severity::Warning,
        message: format!(
            "The fix for `{}` was not applied because it would break the code",
            diagnostic.rule
        ),
        help: Some(
            "Fix this issue by hand. If the fix comes from a custom rule, check its `fix` template."
                .to_string(),
        ),
        span: diagnostic.span,
        fixes: Vec::new(),
    }
}

/// A unified diff between `original` and `fixed`, with `a/<path>` and
/// `b/<path>` headers. Empty when the texts are equal.
pub fn unified_diff(display_path: &str, original: &str, fixed: &str) -> String {
    if original == fixed {
        return String::new();
    }
    // `a/` and `b/` prefixes like git, so `patch -p1` works. Absolute paths
    // are shown as they are.
    let (old_header, new_header) = if Path::new(display_path).is_absolute() {
        (display_path.to_string(), display_path.to_string())
    } else {
        (format!("a/{}", display_path), format!("b/{}", display_path))
    };
    let diff = similar::TextDiff::from_lines(original, fixed);
    diff.unified_diff()
        .header(&old_header, &new_header)
        .to_string()
}
