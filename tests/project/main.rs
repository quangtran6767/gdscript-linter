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
