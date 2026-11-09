#![allow(clippy::unwrap_used)]
use gdscript_formatter::project::{MemberLookup, ProjectIndex, find_root};
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
