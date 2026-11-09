//! Sandboxed JavaScript rules. A provider owns its runtime on one worker.
pub mod bindings;
pub mod runtime;

use crate::linter::api::{Config, RuleLevel};
use crate::linter::config::{CliRuleOverrides, CompiledConfig};
use crate::linter::custom::loader::{RuleTests, parse_tests, validate_id};
use crate::linter::diagnostic::{
    Diagnostic, Edit, FileContext, Fix, FixSafety, RuleMeta, RuleOrigin, RuleProvider, Severity,
    Span,
};
use crate::project::ProjectIndex;
use rquickjs::Function;
use runtime::{Sandbox, describe_error};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct JsRule {
    pub meta: RuleMeta,
    pub tests: RuleTests,
}

pub struct JsRuleProvider {
    pub sandbox: Sandbox,
    pub loaded: Vec<JsRule>,
    metas: Vec<RuleMeta>,
    config: CompiledConfig,
    project: Option<Rc<ProjectIndex>>,
}

impl JsRuleProvider {
    pub fn load(config: &Config, files: &[PathBuf], existing: &[RuleMeta]) -> Result<Self, String> {
        let sandbox = Sandbox::new()?;
        let mut ids = HashMap::new();
        for rule in existing {
            ids.insert(rule.id.clone(), format!("{:?}", rule.origin));
        }
        let mut loaded = Vec::with_capacity(files.len());
        let mut metas = Vec::with_capacity(files.len());
        for (index, path) in files.iter().enumerate() {
            let source = std::fs::read_to_string(path)
                .map_err(|error| format!("{}: {}", path.display(), error))?;
            let metadata = sandbox
                .load(index, &path.to_string_lossy(), &source)
                .map_err(|error| format!("{}: {}", path.display(), error))?;
            let value: Value =
                serde_json::from_str(&metadata).map_err(|error| error.to_string())?;
            let meta = value
                .get("meta")
                .and_then(Value::as_object)
                .ok_or_else(|| format!("{}: `meta`: expected an object", path.display()))?;
            let id = required_string(meta.get("id"), "meta.id", path)?;
            validate_id(path, &id).map_err(|error| error.to_string())?;
            if let Some(first) = ids.insert(id.clone(), path.display().to_string()) {
                return Err(format!(
                    "{}: duplicate rule id `{}` already defined in {}",
                    path.display(),
                    id,
                    first
                ));
            }
            let description = required_string(meta.get("description"), "meta.description", path)?;
            let severity = match meta.get("severity") {
                None => Severity::Warning,
                Some(Value::String(name)) => Severity::from_name(name)
                    .ok_or_else(|| format!("{}: invalid meta.severity", path.display()))?,
                _ => return Err(format!("{}: invalid meta.severity", path.display())),
            };
            if value["hasCreate"] != true {
                return Err(format!("{}: `create`: expected a function", path.display()));
            }
            if let Some(safety) = meta.get("fixSafety") {
                if safety.as_str().and_then(FixSafety::from_name).is_none() {
                    return Err(format!("{}: invalid meta.fixSafety", path.display()));
                }
            }
            if let Some(fixable) = meta.get("fixable") {
                if !fixable.is_boolean() {
                    return Err(format!(
                        "{}: meta.fixable must be a boolean",
                        path.display()
                    ));
                }
            }
            let tests = parse_tests(path, &value["tests"]).map_err(|error| error.to_string())?;
            let meta = RuleMeta {
                id,
                description,
                rationale: meta
                    .get("rationale")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                default_severity: severity,
                fixable: meta
                    .get("fixable")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                bad_example: tests
                    .invalid
                    .first()
                    .map(|case| case.code.clone())
                    .unwrap_or_default(),
                good_example: tests.valid.first().cloned().unwrap_or_default(),
                origin: RuleOrigin::Custom { path: path.clone() },
            };
            metas.push(meta.clone());
            loaded.push(JsRule { meta, tests });
        }
        Ok(Self {
            sandbox,
            loaded,
            metas,
            config: CompiledConfig::new(config.clone()).map_err(|error| error.to_string())?,
            project: None,
        })
    }

    pub fn check_rule(
        &mut self,
        index: usize,
        context: &FileContext,
    ) -> Result<Vec<Diagnostic>, String> {
        self.sandbox.begin();
        self.run_rule(index, context, &Value::Object(Default::default()))
    }

    fn run_rule(
        &mut self,
        index: usize,
        context: &FileContext,
        options: &Value,
    ) -> Result<Vec<Diagnostic>, String> {
        if let Some(project) = context.project {
            if self
                .project
                .as_ref()
                .is_none_or(|cached| cached.root != project.root)
            {
                self.project = Some(Rc::new(project.clone()));
            }
        } else {
            self.project = None;
        }
        let meta = &self.metas[index];
        let raw = self.sandbox.context.with(|ctx| {
            let result = (|| -> rquickjs::Result<String> {
                bindings::install(&ctx, context, self.project.clone())?;
                let prepare: Function = ctx.globals().get("__prepare")?;
                let info = ctx.json_parse(json!({"filename":std::fs::canonicalize(context.path).or_else(|_|std::path::absolute(context.path)).unwrap_or_else(|_|context.path.to_path_buf()).to_string_lossy(),"source":context.source,
                    "hasSemantic":context.semantic.is_some(),"hasProject":context.project.is_some()}).to_string())?;
                let options = ctx.json_parse(options.to_string())?;
                let kinds: Vec<String> = prepare.call((index, info, options))?;
                let dispatch: Function = ctx.globals().get("__dispatch")?;
                let mut cursor = context.tree.walk();
                let mut path: Vec<u32> = Vec::new();
                loop {
                    let node = cursor.node();
                    if kinds.iter().any(|kind| kind == node.kind()) { dispatch.call::<_, ()>((node.kind(), path.clone()))?; }
                    if cursor.goto_first_child() { path.push(0); continue; }
                    loop {
                        if cursor.goto_next_sibling() {
                            if let Some(index) = path.last_mut() { *index += 1; }
                            break;
                        }
                        if !cursor.goto_parent() {
                            let finish: Function = ctx.globals().get("__finish")?;
                            return finish.call(());
                        }
                        path.pop();
                    }
                }
            })();
            result.map_err(|error| describe_error(&ctx, &error))
        }).map_err(|error| format!("{}: {}", meta.id, error))?;
        let values: Vec<Value> = serde_json::from_str(&raw).map_err(|error| error.to_string())?;
        let mut diagnostics = Vec::with_capacity(values.len());
        for value in values {
            let span = parse_range(context.source, &value["range"])?;
            let mut fixes = Vec::new();
            if let Some(edits) = value["edits"].as_array() {
                if !edits.is_empty() {
                    let safety = value["safety"]
                        .as_str()
                        .and_then(FixSafety::from_name)
                        .ok_or("invalid fix safety")?;
                    let mut changes = Vec::with_capacity(edits.len());
                    for edit in edits {
                        changes.push(Edit {
                            span: parse_range(context.source, &edit["range"])?,
                            replacement: edit["replacement"]
                                .as_str()
                                .ok_or("edit replacement must be a string")?
                                .to_string(),
                        });
                    }
                    crate::linter::diagnostic::apply_edits(context.source, &changes)?;
                    fixes.push(Fix {
                        description: value["description"]
                            .as_str()
                            .unwrap_or("Apply JS fix")
                            .to_string(),
                        safety,
                        edits: changes,
                    });
                }
            }
            diagnostics.push(Diagnostic {
                check: None,
                rule: meta.id.clone(),
                severity: meta.default_severity,
                message: value["message"]
                    .as_str()
                    .ok_or("message must be a string")?
                    .to_string(),
                help: value["help"].as_str().map(str::to_string),
                span,
                fixes,
            });
        }
        Ok(diagnostics)
    }
}

impl RuleProvider for JsRuleProvider {
    fn rules(&self) -> &[RuleMeta] {
        &self.metas
    }
    fn check_file(&mut self, context: &FileContext) -> Vec<Diagnostic> {
        self.sandbox.begin();
        let settings = self
            .config
            .settings_for_file(context.path, &CliRuleOverrides::default());
        let mut diagnostics = Vec::new();
        for index in 0..self.metas.len() {
            if settings.levels.get(&self.metas[index].id) == Some(&RuleLevel::Off) {
                continue;
            }
            let options = match settings.rule_options.get(&self.metas[index].id) {
                Some(table) => serde_json::to_value(table).unwrap_or(Value::Null),
                None => json!({}),
            };
            match self.run_rule(index, context, &options) {
                Ok(issues) => diagnostics.extend(issues),
                Err(message) => diagnostics.push(Diagnostic {
                    check: None,
                    rule: "script-error".to_string(),
                    severity: Severity::Error,
                    message,
                    help: Some(
                        "Fix the JavaScript rule or its resource usage and lint again.".to_string(),
                    ),
                    span: Span::from_byte_range(context.source, 0, 0),
                    fixes: Vec::new(),
                }),
            }
        }
        self.sandbox.collect_garbage();
        diagnostics
    }
}

fn required_string(
    value: Option<&Value>,
    key: &str,
    path: &std::path::Path,
) -> Result<String, String> {
    match value.and_then(Value::as_str) {
        Some(text) if !text.is_empty() => Ok(text.to_string()),
        _ => Err(format!(
            "{}: `{}`: expected a nonempty string",
            path.display(),
            key
        )),
    }
}

fn parse_range(source: &str, value: &Value) -> Result<Span, String> {
    let start = value["start"]["byte"]
        .as_u64()
        .and_then(|byte| usize::try_from(byte).ok())
        .ok_or("range.start.byte must be an integer")?;
    let end = value["end"]["byte"]
        .as_u64()
        .and_then(|byte| usize::try_from(byte).ok())
        .ok_or("range.end.byte must be an integer")?;
    if start > end
        || end > source.len()
        || !source.is_char_boundary(start)
        || !source.is_char_boundary(end)
    {
        return Err("invalid UTF-8 byte range from JavaScript rule".to_string());
    }
    Ok(Span::from_byte_range(source, start, end))
}

/// Runs embedded JS tests against only their owning rule, at all fix safeties.
pub fn test_rules(mut provider: JsRuleProvider) -> Vec<crate::linter::rule_tester::RuleReport> {
    let cases = provider.loaded.clone();
    let mut reports = Vec::with_capacity(cases.len());
    for (index, rule) in cases.iter().enumerate() {
        let mut report = crate::linter::rule_tester::RuleReport {
            id: rule.meta.id.clone(),
            passed: 0,
            failures: Vec::new(),
        };
        for (case_index, code) in rule.tests.valid.iter().enumerate() {
            match check_test(&mut provider, index, code) {
                Ok(diagnostics) if diagnostics.is_empty() => report.passed += 1,
                Ok(diagnostics) => report.failures.push(format!(
                    "valid[{}]: expected no diagnostics, got {}",
                    case_index,
                    diagnostics.len()
                )),
                Err(error) => report
                    .failures
                    .push(format!("valid[{}]: {}", case_index, error)),
            }
        }
        for (case_index, case) in rule.tests.invalid.iter().enumerate() {
            let result = (|| -> Result<(), String> {
                let diagnostics = check_test(&mut provider, index, &case.code)?;
                let correct = match case.errors {
                    Some(count) => diagnostics.len() == count,
                    None => !diagnostics.is_empty(),
                };
                if !correct {
                    return Err(format!(
                        "expected {:?} diagnostics (None means at least one), got {}",
                        case.errors,
                        diagnostics.len()
                    ));
                }
                if let Some(expected) = &case.output {
                    let mut edits = Vec::new();
                    for diagnostic in &diagnostics {
                        if let Some(fix) = diagnostic.fixes.first() {
                            edits.extend(fix.edits.clone());
                        }
                    }
                    let fixed = crate::linter::diagnostic::apply_edits(&case.code, &edits)?;
                    if fixed != *expected {
                        return Err(format!(
                            "fixed output differs: expected {:?}, got {:?}",
                            expected, fixed
                        ));
                    }
                    if crate::linter::rule_tester::parse(&fixed)?
                        .root_node()
                        .has_error()
                    {
                        return Err("fixed output does not parse".to_string());
                    }
                    if !check_test(&mut provider, index, &fixed)?.is_empty() {
                        return Err("fix is not idempotent".to_string());
                    }
                }
                Ok(())
            })();
            match result {
                Ok(()) => report.passed += 1,
                Err(error) => report
                    .failures
                    .push(format!("invalid[{}]: {}", case_index, error)),
            }
        }
        reports.push(report);
    }
    reports
}

fn check_test(
    provider: &mut JsRuleProvider,
    index: usize,
    source: &str,
) -> Result<Vec<Diagnostic>, String> {
    let tree = crate::linter::rule_tester::parse(source)?;
    let semantic = crate::semantic::SemanticModel::build(&tree, source);
    provider.check_rule(
        index,
        &FileContext {
            path: std::path::Path::new("test.gd"),
            source,
            tree: &tree,
            semantic: Some(&semantic),
            project: None,
        },
    )
}
