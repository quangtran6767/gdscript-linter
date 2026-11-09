// The GodotClass derive generates `base: base` in its init code, which newer
// clippy versions flag. The lint fires on macro output we don't control.
#![allow(clippy::redundant_field_names)]

use gdguard::linter::diagnostic::{Diagnostic, Severity, Span};
use gdguard::linter::editor::{EditorDiagnostic, EditorSession, payloads};
use gdguard::linter::{LinterConfig, lint_gdscript_with_config};
use gdguard::{FormatterConfiguration, PrinterConfiguration, QuoteStyle, format_gdscript};
use godot::builtin::{Array, PackedStringArray};
use godot::obj::Singleton;
use godot::prelude::{
    Base, Dictionary, ExtensionLibrary, GString, GodotClass, Object, Variant, gdextension,
    godot_api, godot_error,
};
use std::cell::RefCell;
use std::path::Path;

struct GdguardExtension;

#[gdextension]
unsafe impl ExtensionLibrary for GdguardExtension {}

#[derive(GodotClass)]
#[class(init, singleton, base=Object)]
struct GDGuard {
    base: Base<Object>,
    session: RefCell<EditorSession>,
}

#[godot_api]
impl GDGuard {
    #[func]
    pub fn format_gdscript(
        &self,
        source: GString,
        config: Dictionary<Variant, Variant>,
    ) -> GString {
        let formatter_config = dict_to_formatter_config(&config);
        match format_gdscript(&source.to_string(), &formatter_config) {
            Ok(formatted) => GString::from(&formatted),
            Err(error) => {
                godot_error!("Formatter error: {}", error);
                GString::new()
            }
        }
    }

    #[func]
    pub fn lint(&self, path: GString, source: GString) -> Array<Dictionary<Variant, Variant>> {
        let absolute = godot::classes::ProjectSettings::singleton()
            .globalize_path(&path)
            .to_string();
        let source = source.to_string();
        match self
            .session
            .borrow_mut()
            .lint(Path::new(&absolute), &source)
        {
            Ok(issues) => payloads_to_array(&issues),
            Err(error) => {
                godot_error!("Linter error: {}", error);
                issues_to_array(&[editor_error(&source, error)])
            }
        }
    }

    #[func]
    pub fn fix(
        &self,
        path: GString,
        source: GString,
        unsafe_fixes: bool,
    ) -> Dictionary<Variant, Variant> {
        let absolute = godot::classes::ProjectSettings::singleton()
            .globalize_path(&path)
            .to_string();
        let source = source.to_string();
        let mut result = Dictionary::new();
        match self
            .session
            .borrow_mut()
            .fix(Path::new(&absolute), &source, unsafe_fixes)
        {
            Ok(fixed) => {
                result.set("output", &GString::from(&fixed.output));
                result.set("applied", fixed.applied as i64);
                result.set("remaining", &payloads_to_array(&fixed.remaining));
            }
            Err(error) => {
                godot_error!("Linter error: {}", error);
                result.set("output", &GString::from(&source));
                result.set("applied", 0_i64);
                result.set("error", &GString::from(&error));
                result.set(
                    "remaining",
                    &issues_to_array(&[editor_error(&source, error)]),
                );
            }
        }
        result
    }

    /// Applies one reviewed fix atomically, rejecting malformed ranges or code.
    #[func]
    pub fn apply_edits(
        &self,
        source: GString,
        edits: Array<Dictionary<Variant, Variant>>,
    ) -> Dictionary<Variant, Variant> {
        let source = source.to_string();
        let mut result = Dictionary::new();
        let output = (|| -> Result<String, String> {
            let mut changes = Vec::with_capacity(edits.len());
            for edit in edits.iter_shared() {
                let start = edit
                    .get("start_byte")
                    .and_then(|value| value.try_to::<i64>().ok())
                    .and_then(|value| usize::try_from(value).ok())
                    .ok_or("invalid edit start_byte")?;
                let end = edit
                    .get("end_byte")
                    .and_then(|value| value.try_to::<i64>().ok())
                    .and_then(|value| usize::try_from(value).ok())
                    .ok_or("invalid edit end_byte")?;
                if start > end || end > source.len() {
                    return Err("edit is outside source".to_string());
                }
                let replacement = edit
                    .get("replacement")
                    .and_then(|value| value.try_to::<GString>().ok())
                    .ok_or("invalid replacement")?
                    .to_string();
                changes.push(gdguard::linter::diagnostic::Edit {
                    span: Span::from_byte_range(&source, start, end),
                    replacement,
                });
            }
            gdguard::linter::editor::apply_edits_checked(&source, &changes)
        })();
        match output {
            Ok(output) => result.set("output", &GString::from(&output)),
            Err(error) => {
                result.set("output", &GString::from(&source));
                result.set("error", &GString::from(&error));
            }
        }
        result
    }

    /// Deprecated: use lint(path, source) for project configuration and fixes.
    #[func]
    pub fn lint_gdscript(
        &self,
        source: GString,
        config: Dictionary<Variant, Variant>,
    ) -> Array<Dictionary<Variant, Variant>> {
        let linter_config = dict_to_linter_config(&config);
        match lint_gdscript_with_config(&source.to_string(), "", &linter_config) {
            Ok(issues) => issues_to_array(&issues),
            Err(error) => {
                godot_error!("Linter error: {}", error);
                Array::<Dictionary<Variant, Variant>>::new()
            }
        }
    }
}

macro_rules! extract_field {
    ($dict:expr, $key:ident, $type:ty, $target:expr) => {
        if let Some(variant) = $dict.get(stringify!($key)) {
            match variant.try_to::<$type>() {
                Ok(val) => $target.$key = val,
                Err(error) => {
                    godot_error!("Config '{}' is invalid: {}", stringify!($key), error);
                }
            }
        }
    };
    // Special case for usize
    ($dict:expr, $key:ident, $type:ty, as usize, $target:expr) => {
        if let Some(variant) = $dict.get(stringify!($key)) {
            match variant.try_to::<$type>() {
                Ok(val) => $target.$key = val as usize,
                Err(error) => {
                    godot_error!("Config '{}' is invalid: {}", stringify!($key), error);
                }
            }
        }
    };
}

fn dict_to_formatter_config(dict: &Dictionary<Variant, Variant>) -> FormatterConfiguration {
    let mut result = FormatterConfiguration::default();
    extract_field!(dict, safe, bool, result);
    extract_field!(dict, reorder_code, bool, result);
    extract_field!(dict, blank_lines_around_definitions, u16, result);
    if let Some(variant) = dict.get("printer") {
        match variant.try_to::<Dictionary<Variant, Variant>>() {
            Ok(sub_dict) => result.printer = dict_to_printer_config(&sub_dict),
            Err(error) => {
                godot_error!("Config 'printer' is invalid: {}", error);
            }
        }
    }
    if let Some(variant) = dict.get("quote_style") {
        match variant
            .try_to::<GString>()
            .ok()
            .and_then(|gstr| QuoteStyle::from_name(&gstr.to_string()))
        {
            Some(valid_style) => result.quote_style = valid_style,
            None => {
                godot_error!("Config 'quote_style' is invalid");
            }
        }
    }
    result
}

fn dict_to_printer_config(dict: &Dictionary<Variant, Variant>) -> PrinterConfiguration {
    let mut result = PrinterConfiguration::default();
    extract_field!(dict, max_line_length, i64, as usize, result);
    extract_field!(dict, indent_size, i64, as usize, result);
    extract_field!(dict, use_spaces, bool, result);
    extract_field!(dict, insert_final_newline, bool, result);
    extract_field!(dict, trim_trailing_whitespace, bool, result);
    extract_field!(dict, indent_blank_lines, bool, result);
    extract_field!(dict, maximum_blank_lines, u16, result);
    extract_field!(dict, continuation_indent_level, u16, result);
    result
}

fn dict_to_linter_config(dict: &Dictionary<Variant, Variant>) -> LinterConfig {
    let mut result = LinterConfig::default();
    extract_field!(dict, max_line_length, i64, as usize, result);
    if let Some(variant) = dict.get("disabled_rules") {
        match variant.try_to::<PackedStringArray>() {
            Ok(rules) => {
                result.disabled_rules = rules
                    .as_slice()
                    .iter()
                    .map(|gd_str| gd_str.to_string())
                    .collect()
            }
            Err(error) => {
                godot_error!("Config 'disabled_rules' is invalid: {}", error);
            }
        }
    }
    result
}

fn editor_error(source: &str, message: String) -> Diagnostic {
    Diagnostic {
        rule: "config-error".to_string(),
        severity: Severity::Error,
        message,
        help: Some("Fix the project configuration or custom rule and lint again.".to_string()),
        span: Span::from_byte_range(source, 0, 0),
        fixes: Vec::new(),
    }
}

fn issues_to_array(diagnostics: &[Diagnostic]) -> Array<Dictionary<Variant, Variant>> {
    payloads_to_array(&payloads(diagnostics))
}

fn payloads_to_array(issues: &[EditorDiagnostic]) -> Array<Dictionary<Variant, Variant>> {
    let mut result = Array::new();
    for issue in issues {
        let mut item = Dictionary::<Variant, Variant>::new();
        item.set("rule", &GString::from(&issue.rule));
        item.set("severity", &GString::from(&issue.severity));
        item.set("message", &GString::from(&issue.message));
        item.set("help", &GString::from(issue.help.as_deref().unwrap_or("")));
        item.set("line", issue.line as i64);
        item.set("column", issue.column as i64);
        item.set("end_line", issue.end_line as i64);
        item.set("end_column", issue.end_column as i64);
        item.set("start_byte", issue.start_byte as i64);
        item.set("end_byte", issue.end_byte as i64);
        let mut fixes = Array::<Dictionary<Variant, Variant>>::new();
        for fix in &issue.fixes {
            let mut payload = Dictionary::<Variant, Variant>::new();
            payload.set("description", &GString::from(&fix.description));
            payload.set("safety", &GString::from(&fix.safety));
            let mut edits = Array::<Dictionary<Variant, Variant>>::new();
            for edit in &fix.edits {
                let mut change = Dictionary::<Variant, Variant>::new();
                change.set("start_byte", edit.span.start_byte as i64);
                change.set("end_byte", edit.span.end_byte as i64);
                change.set("replacement", &GString::from(&edit.replacement));
                edits.push(&change);
            }
            payload.set("edits", &edits);
            fixes.push(&payload);
        }
        item.set("fixes", &fixes);
        result.push(&item);
    }
    result
}
