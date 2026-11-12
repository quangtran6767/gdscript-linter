//! Run with GDGUARD_TEST_GODOT=/path/to/godot cargo test --test godot_check -- --ignored.
use gdguard::linter::godot_check::check_files;
use std::path::PathBuf;

#[test]
#[ignore = "requires a Godot 4 executable in GDGUARD_TEST_GODOT"]
fn autoload_access_compiles_and_real_type_errors_are_still_reported() {
    let executable =
        PathBuf::from(std::env::var_os("GDGUARD_TEST_GODOT").expect("Godot executable"));
    let root = std::env::temp_dir().join(format!("gdguard-autoload-check-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("project directory");
    for (name, source) in [
        (
            "project.godot",
            "config_version=5\n[autoload]\nDemoStore=\"*res://demo_store.gd\"\n",
        ),
        (
            "demo_store.gd",
            "extends Node\nsignal changed(total: int)\nvar total := 0\n",
        ),
        (
            "demo.gd",
            "extends Node\nfunc _ready() -> void:\n\tDemoStore.changed.connect(_on_changed)\n\t_on_changed(DemoStore.total)\nfunc _on_changed(total: int) -> void:\n\tprint(total)\n",
        ),
        ("bad.gd", "extends Node\nvar value: int = \"wrong type\"\n"),
    ] {
        std::fs::write(root.join(name), source).expect("project file");
    }
    let good = root.join("demo.gd");
    let autoload = root.join("demo_store.gd");
    let bad = root.join("bad.gd");
    let results = check_files(&executable, &[good.clone(), autoload.clone(), bad.clone()])
        .expect("Godot check");
    assert!(results.get(&good).is_none_or(Vec::is_empty), "{results:?}");
    assert!(
        results.get(&autoload).is_none_or(Vec::is_empty),
        "{results:?}"
    );
    let errors = results.get(&bad).expect("actual type error");
    assert!(!errors.is_empty());
    for error in errors {
        assert_eq!(error.rule, "godot-error");
        assert_eq!(error.span.start.line, 2);
        assert!(error.message.contains("String"), "{errors:?}");
    }
    std::fs::remove_dir_all(root).expect("remove project");
}
