use std::fs;
use std::process::Command;

#[test]
fn custom_rule_documentation_examples_pass_the_rule_tester() {
    let documentation = include_str!("../../docs/custom_rules.md");
    let root = std::env::temp_dir().join(format!("gdguard-doc-rules-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let mut count = 0;
    for block in documentation.split("```yaml\n").skip(1) {
        let rule = block.split_once("```").unwrap().0;
        fs::write(root.join(format!("example-{}.yml", count)), rule).unwrap();
        count += 1;
    }
    assert_eq!(count, 6);
    let output = Command::new(env!("CARGO_BIN_EXE_gdscript-formatter"))
        .arg("test-rules")
        .arg(&root)
        .output()
        .unwrap();
    fs::remove_dir_all(root).unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
