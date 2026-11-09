#![allow(clippy::unwrap_used)]
use gdguard::linter::api;
use gdguard::linter::diagnostic::{Diagnostic, Edit, Fix, FixSafety, Severity, Span};
use gdguard::linter::editor::{EditorSession, apply_edits_checked, payloads};
use std::path::Path;

#[test]
fn payload_conversion_preserves_unicode_spans_help_and_fixes() {
    let source = "var text = '😀漢'\n";
    let start = source.find('😀').unwrap();
    let span = Span::from_byte_range(source, start, start + 4);
    let diagnostic = Diagnostic {
        check: None,
        rule: "test-rule".to_string(),
        severity: Severity::Warning,
        message: "message".to_string(),
        help: Some("help".to_string()),
        span,
        fixes: vec![Fix {
            description: "replace emoji".to_string(),
            safety: FixSafety::Safe,
            edits: vec![Edit {
                span,
                replacement: "字".to_string(),
            }],
        }],
    };
    let result = payloads(&[diagnostic]);
    let item = &result[0];
    assert_eq!(item.line, 1);
    assert_eq!(item.column, start + 1);
    assert_eq!(item.end_column, start + 5);
    assert_eq!(item.start_byte, start);
    assert_eq!(item.end_byte, start + 4);
    assert_eq!(item.help.as_deref(), Some("help"));
    assert_eq!(item.fixes[0].safety, "safe");
    assert_eq!(
        apply_edits_checked(source, &item.fixes[0].edits).unwrap(),
        "var text = '字漢'\n"
    );
    let bad = Edit {
        span: Span::from_byte_range(source, 0, source.len()),
        replacement: "func broken(:".to_string(),
    };
    assert!(apply_edits_checked(source, &[bad]).is_err());
}

#[test]
fn editor_session_matches_library_on_all_sample_files() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/e2e/sample_project");
    let mut session = EditorSession::default();
    fn visit(root: &Path, session: &mut EditorSession) {
        for entry in std::fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                if path.file_name().unwrap() != ".gdguard" {
                    visit(&path, session);
                }
            } else if path.extension().is_some_and(|extension| extension == "gd") {
                let source = std::fs::read_to_string(&path).unwrap();
                let config = api::load_config_for(&path).unwrap();
                let expected = api::lint_source(&path, &source, &config).unwrap();
                assert_eq!(
                    session.lint(&path, &source).unwrap(),
                    payloads(&expected.diagnostics)
                );
            }
        }
    }
    visit(&root, &mut session);
}

#[test]
fn editor_config_cache_refreshes_after_same_session_edits() {
    let root = std::env::temp_dir().join(format!("gdguard-editor-config-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let config = root.join("gdguard.toml");
    std::fs::write(&config, "[rules]\nvariable-name='error'\n").unwrap();
    let path = root.join("main.gd");
    let source = "var badName: int = 1\n";
    let mut session = EditorSession::default();
    assert!(
        session
            .lint(&path, source)
            .unwrap()
            .iter()
            .any(|issue| issue.check_id() == "variable-name")
    );
    std::fs::write(config, "[rules]\nvariable-name='off'\n").unwrap();
    assert!(session.lint(&path, source).unwrap().is_empty());
    std::fs::remove_dir_all(root).unwrap();
}
