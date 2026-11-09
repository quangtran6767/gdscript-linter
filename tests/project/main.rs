#![allow(clippy::unwrap_used)]
use gdguard::project::{MemberLookup, ProjectIndex, find_root};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Project {
    root: PathBuf,
}
impl Project {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "gdguard-project-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("scripts")).unwrap();
        std::fs::write(
            root.join("project.godot"),
            "config_version=5\n[autoload]\nGame=\"*res://scripts/game.gd\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("scripts/base.gd"),
            "class_name Base\nvar inherited = 1\nsignal changed\nfunc act():\n\tpass\n",
        )
        .unwrap();
        std::fs::write(
            root.join("scripts/middle.gd"),
            "class_name Middle\nextends Base\n",
        )
        .unwrap();
        std::fs::write(
            root.join("scripts/child.gd"),
            "class_name Child\nextends \"middle.gd\"\nvar own = 2\n",
        )
        .unwrap();
        std::fs::write(
            root.join("scripts/game.gd"),
            "class_name GameScript\nextends Node\nvar score = 0\n",
        )
        .unwrap();
        Self { root }
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
#[test]
fn autoloads_summaries_and_three_level_inheritance() {
    let project = Project::new();
    assert_eq!(
        find_root(&project.root.join("scripts/child.gd")),
        Some(project.root.clone())
    );
    let index = ProjectIndex::build(&project.root).unwrap();
    assert_eq!(index.scripts.len(), 4);
    assert_eq!(index.autoloads["Game"], PathBuf::from("scripts/game.gd"));
    for member in ["own", "inherited", "act", "changed"] {
        assert_eq!(index.has_member("Child", member), MemberLookup::Yes);
    }
    assert_eq!(index.has_member("Child", "missing"), MemberLookup::No);
    assert_eq!(
        index.has_member("GameScript", "position"),
        MemberLookup::Unknown
    );
    assert_eq!(
        index.has_member("MissingClass", "anything"),
        MemberLookup::Unknown
    );
}
#[test]
fn cache_reuses_stable_content_and_invalidates_edits_and_deleted_files() {
    let project = Project::new();
    ProjectIndex::build(&project.root).unwrap();
    let path = project.root.join(".gdguard/cache/index.json");
    let first = std::fs::read(&path).unwrap();
    ProjectIndex::build(&project.root).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), first);
    std::fs::write(
        project.root.join("scripts/base.gd"),
        "class_name Base\nvar added = 1\n",
    )
    .unwrap();
    let index = ProjectIndex::build(&project.root).unwrap();
    assert_eq!(index.has_member("Child", "added"), MemberLookup::Yes);
    assert_eq!(index.has_member("Child", "inherited"), MemberLookup::No);
    assert_ne!(std::fs::read(&path).unwrap(), first);
    std::fs::remove_file(project.root.join("scripts/base.gd")).unwrap();
    assert_eq!(
        ProjectIndex::build(&project.root)
            .unwrap()
            .has_member("Child", "added"),
        MemberLookup::Unknown
    );
}
#[test]
fn cycles_and_broken_scripts_do_not_produce_false_missing_members() {
    let project = Project::new();
    std::fs::write(
        project.root.join("scripts/base.gd"),
        "class_name Base\nextends Child\n",
    )
    .unwrap();
    assert_eq!(
        ProjectIndex::build(&project.root)
            .unwrap()
            .has_member("Child", "missing"),
        MemberLookup::Unknown
    );
    std::fs::write(
        project.root.join("scripts/base.gd"),
        "class_name Base\nfunc broken(:\n",
    )
    .unwrap();
    assert_eq!(
        ProjectIndex::build(&project.root)
            .unwrap()
            .has_member("Child", "missing"),
        MemberLookup::Unknown
    );
}

#[test]
fn cross_file_rules_count_scene_and_script_usage_and_skip_unknown_engine_members() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/project/fixtures/cross");
    let config = gdguard::linter::api::Config::default();
    let data = root.join("data.gd");
    let source = std::fs::read_to_string(&data).unwrap();
    let diagnostics = gdguard::linter::api::lint_source(&data, &source, &config)
        .unwrap()
        .diagnostics;
    let unused: Vec<_> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.rule == "unused-signal")
        .collect();
    assert_eq!(unused.len(), 1, "{:?}", diagnostics);
    assert!(unused[0].message.contains("never_used"));
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule == "unused-local-signal")
    );
    let caller = root.join("caller.gd");
    let source = std::fs::read_to_string(&caller).unwrap();
    let diagnostics = gdguard::linter::api::lint_source(&caller, &source, &config)
        .unwrap()
        .diagnostics;
    let missing: Vec<_> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.rule == "unknown-member-on-project-class")
        .collect();
    assert_eq!(missing.len(), 1);
    assert_eq!(
        &source[missing[0].span.start_byte..missing[0].span.end_byte],
        "missing"
    );
}

#[test]
fn dynamic_signal_emission_is_conservatively_unknown() {
    let project = Project::new();
    std::fs::write(
        project.root.join("dynamic.gd"),
        "signal unknown_usage\nfunc f(signal_name):\n\temit_signal(signal_name)\n",
    )
    .unwrap();
    let index = ProjectIndex::build(&project.root).unwrap();
    let analysis = gdguard::project::ProjectAnalysis::build(&index).unwrap();
    assert!(analysis.dynamic_signal_usage);
}

#[test]
fn live_buffer_signal_uses_override_stale_disk_usage() {
    let project = Project::new();
    let path = project.root.join("live.gd");
    std::fs::write(&path, "signal newly_used\n").unwrap();
    let source = "signal newly_used\nfunc f():\n\temit_signal(\"newly_used\")\n";
    let issues = gdguard::linter::api::lint_source(&path, source, &Default::default())
        .unwrap()
        .diagnostics;
    assert!(!issues.iter().any(|issue| issue.rule == "unused-signal"));
}

/// A throwaway Godot project for the W9 rules.
struct TempProject {
    root: PathBuf,
}
impl TempProject {
    fn new(project_godot: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "gdguard-w9-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("project.godot"), project_godot).unwrap();
        Self { root }
    }
    fn write(&self, path: &str, text: &str) -> PathBuf {
        let path = self.root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }
    /// Lints one file of the project and returns `(rule, covered text)`.
    fn lint(&self, path: &str) -> Vec<(String, String)> {
        let full = self.root.join(path);
        let source = std::fs::read_to_string(&full).unwrap();
        let diagnostics = gdguard::linter::api::lint_source(&full, &source, &Default::default())
            .unwrap()
            .diagnostics;
        let mut found = Vec::new();
        for diagnostic in diagnostics {
            let text = source[diagnostic.span.start_byte..diagnostic.span.end_byte].to_string();
            found.push((diagnostic.rule, text));
        }
        found
    }
    fn rules(&self, path: &str, rule: &str) -> Vec<String> {
        let mut texts = Vec::new();
        for (found_rule, text) in self.lint(path) {
            if found_rule == rule {
                texts.push(text);
            }
        }
        texts
    }
}
impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn duplicate_class_names_are_reported_on_each_file_and_left_unresolved() {
    let project = TempProject::new("config_version=5\n");
    project.write("a.gd", "class_name Item\nvar price = 1\n");
    project.write("b.gd", "class_name Item\nvar cost = 1\n");
    project.write(
        "user.gd",
        "class_name User\nfunc f(item: Item):\n\treturn item.missing\n",
    );
    // The index builds, and lookups through the ambiguous name are Unknown.
    let index = ProjectIndex::build_uncached(&project.root).unwrap();
    assert_eq!(index.scripts.len(), 3);
    assert_eq!(index.has_member("Item", "missing"), MemberLookup::Unknown);
    assert_eq!(index.scripts_declaring("Item").len(), 2);
    assert_eq!(project.rules("a.gd", "duplicate-class-name"), vec!["Item"]);
    assert_eq!(project.rules("b.gd", "duplicate-class-name"), vec!["Item"]);
    assert!(
        project
            .rules("user.gd", "unknown-member-on-project-class")
            .is_empty()
    );
}

#[test]
fn missing_class_name_skips_autoload_scripts_and_scenes_and_offers_a_free_name() {
    let project = TempProject::new(
        "config_version=5\n[autoload]\nGame=\"*res://game.gd\"\nHud=\"*res://hud.tscn\"\n",
    );
    project.write("game.gd", "extends Node\n");
    project.write("hud_root.gd", "extends CanvasLayer\n");
    project.write(
        "hud.tscn",
        "[gd_scene format=3]\n[ext_resource type=\"Script\" path=\"res://hud_root.gd\" id=\"1\"]\n[node name=\"Hud\" type=\"CanvasLayer\"]\nscript = ExtResource(\"1\")\n",
    );
    project.write(
        "player_controller.gd",
        "@tool\nextends Node2D\n\nvar speed = 1\n",
    );
    project.write("node.gd", "extends Node\n");
    project.write("taken.gd", "class_name Taken\n");
    project.write("other/taken.gd", "extends Node\n");
    project.write("named.gd", "class_name Named\nextends Node\n");
    assert!(project.rules("game.gd", "missing-class-name").is_empty());
    assert!(
        project
            .rules("hud_root.gd", "missing-class-name")
            .is_empty()
    );
    assert!(project.rules("named.gd", "missing-class-name").is_empty());
    assert_eq!(
        project.rules("player_controller.gd", "missing-class-name"),
        vec!["extends Node2D"]
    );

    let fixed = |path: &str| -> Option<String> {
        let full = project.root.join(path);
        let source = std::fs::read_to_string(&full).unwrap();
        let diagnostics = gdguard::linter::api::lint_source(&full, &source, &Default::default())
            .unwrap()
            .diagnostics;
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.rule == "missing-class-name")?;
        let fix = diagnostic.fixes.first()?;
        assert_eq!(fix.safety, gdguard::linter::diagnostic::FixSafety::Unsafe);
        Some(gdguard::linter::diagnostic::apply_edits(&source, &fix.edits).unwrap())
    };
    assert_eq!(
        fixed("player_controller.gd").unwrap(),
        "@tool\nclass_name PlayerController\nextends Node2D\n\nvar speed = 1\n"
    );
    // `Node` is an engine class and `Taken` is used: no name to offer.
    assert_eq!(fixed("node.gd"), None);
    assert_eq!(fixed("other/taken.gd"), None);
    assert_eq!(project.rules("node.gd", "missing-class-name").len(), 1);

    // Outside a Godot project the rule has nothing to check against.
    let loose = gdguard::linter::lint_gdscript("extends Node\n", "loose.gd").unwrap();
    assert!(
        !loose
            .iter()
            .any(|diagnostic| diagnostic.rule == "missing-class-name")
    );
}

#[test]
fn scene_connections_report_missing_methods_and_count_as_uses() {
    let project = TempProject::new("config_version=5\n");
    project.write(
        "base.gd",
        "class_name BaseUi\nextends Control\n\nfunc _on_inherited():\n\tpass\n",
    );
    project.write(
        "ui.gd",
        "class_name Ui\nextends BaseUi\n\nfunc _on_button_pressed():\n\tpass\n\n\nfunc _on_unused():\n\tpass\n",
    );
    project.write("plain.gd", "class_name Plain\n\nfunc other():\n\tpass\n");
    project.write(
        "ui.tscn",
        concat!(
            "[gd_scene format=3 uid=\"uid://cuiscene\"]\n\n",
            "[ext_resource type=\"Script\" path=\"res://ui.gd\" id=\"1_ui\"]\n",
            "[ext_resource type=\"Script\" path=\"res://plain.gd\" id=\"2_plain\"]\n\n",
            "[node name=\"Ui\" type=\"Control\"]\n",
            "script = ExtResource(\"1_ui\")\n\n",
            "[node name=\"Button\" type=\"Button\" parent=\".\"]\n\n",
            "[node name=\"Data\" type=\"Node\" parent=\"Button\"]\n",
            "script = ExtResource(\"2_plain\")\n\n",
            "[connection signal=\"pressed\" from=\"Button\" to=\".\" method=\"_on_button_pressed\"]\n",
            "[connection signal=\"pressed\" from=\"Button\" to=\".\" method=\"_on_inherited\"]\n",
            "[connection signal=\"pressed\" from=\"Button\" to=\".\" method=\"_on_typo_pressed\"]\n",
            "[connection signal=\"pressed\" from=\"Button\" to=\".\" method=\"queue_free\"]\n",
            "[connection signal=\"ready\" from=\".\" to=\"Button/Data\" method=\"missing_plain\"]\n",
            "[connection signal=\"ready\" from=\".\" to=\"Button/Data\" method=\"other\"]\n",
        ),
    );
    let found = project.lint("ui.gd");
    let missing: Vec<&(String, String)> = found
        .iter()
        .filter(|(rule, _)| rule == "scene-connection-missing-method")
        .collect();
    assert_eq!(missing.len(), 1, "{:?}", found);
    assert_eq!(missing[0].1, "extends BaseUi");
    // Connected in the scene, so not unused; `_on_unused` still is.
    assert_eq!(
        project.rules("ui.gd", "unused-private-function"),
        vec!["_on_unused"]
    );
    // `plain.gd` has no engine base: a missing public method is certain.
    let diagnostics = gdguard::linter::api::lint_source(
        &project.root.join("plain.gd"),
        &std::fs::read_to_string(project.root.join("plain.gd")).unwrap(),
        &Default::default(),
    )
    .unwrap()
    .diagnostics;
    let messages: Vec<&str> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.rule == "scene-connection-missing-method")
        .map(|diagnostic| diagnostic.message.as_str())
        .collect();
    assert_eq!(messages.len(), 1);
    assert!(messages[0].contains("'missing_plain'"));
}

#[test]
fn resource_paths_must_exist_and_prefer_known_uids() {
    let project = TempProject::new("config_version=5\n");
    project.write("data.gd", "class_name Data\n");
    project.write("data.gd.uid", "uid://cdatascript\n");
    project.write("art/icon.png", "png");
    project.write(
        "art/icon.png.import",
        "[remap]\nimporter=\"texture\"\nuid=\"uid://bicon\"\npath=\"res://.godot/imported/icon.ctex\"\n",
    );
    project.write(
        "levels/level.tscn",
        "[gd_scene format=3 uid=\"uid://clevel\"]\n",
    );
    project.write("levels/old.tres", "[gd_resource type=\"Theme\" format=3]\n");
    project.write("notes.json", "{}");
    project.write(
        "levels/loader.gd",
        concat!(
            "class_name Loader\n",
            "const DATA = preload(\"res://data.gd\")\n",
            "const ICON = preload('../art/icon.png')\n",
            "var level = load(\"level.tscn\")\n",
            "var theme = load(\"res://levels/old.tres\")\n",
            "var notes = ResourceLoader.load(\"res://notes.json\")\n",
            "var good = load(\"uid://bicon\")\n",
            "var gone = load(\"res://levels/missing.tscn\")\n",
            "var bad_uid = preload(\"uid://cmissing\")\n",
            "var lost = load(\"../../outside.png\")\n",
            "var saved = load(\"user://save.tres\")\n",
        ),
    );
    assert_eq!(
        project.rules("levels/loader.gd", "prefer-uid-path"),
        vec!["\"res://data.gd\"", "'../art/icon.png'", "\"level.tscn\""]
    );
    assert_eq!(
        project.rules("levels/loader.gd", "missing-resource-path"),
        vec!["\"res://levels/missing.tscn\"", "\"uid://cmissing\""]
    );
    // The fix keeps the quote style and is safe.
    let full = project.root.join("levels/loader.gd");
    let source = std::fs::read_to_string(&full).unwrap();
    let diagnostics = gdguard::linter::api::lint_source(&full, &source, &Default::default())
        .unwrap()
        .diagnostics;
    let icon = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic.rule == "prefer-uid-path" && diagnostic.message.contains("icon")
        })
        .unwrap();
    assert_eq!(
        icon.fixes[0].safety,
        gdguard::linter::diagnostic::FixSafety::Safe
    );
    assert_eq!(icon.fixes[0].edits[0].replacement, "'uid://bicon'");
}

#[test]
fn fixes_that_repair_every_syntax_error_are_applied() {
    let project = TempProject::new("config_version=5\n");
    project.write(
        "old.gd",
        "tool\nclass_name Old\nextends Node\n\nonready var label = $Label\n",
    );
    project.write(
        "partial.gd",
        "class_name Partial\nextends Node\n\nonready var label = $Label\nvar hp = 1 setget set_hp\n",
    );
    let fix = |path: &str| -> String {
        let full = project.root.join(path);
        let source = std::fs::read_to_string(&full).unwrap();
        gdguard::linter::api::fix_source(&full, &source, &Default::default(), false)
            .unwrap()
            .output
    };
    assert_eq!(
        fix("old.gd"),
        "@tool\nclass_name Old\nextends Node\n\n@onready var label = $Label\n"
    );
    // `setget` has no fix, so the file keeps a syntax error: left unchanged.
    assert_eq!(
        fix("partial.gd"),
        std::fs::read_to_string(project.root.join("partial.gd")).unwrap()
    );
}
