#![allow(clippy::unwrap_used, clippy::needless_pass_by_value)]
use gdguard::lsp::{byte_to_position, position_to_byte};
use serde_json::{Value, json};
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

struct Client {
    child: Child,
    input: ChildStdin,
    messages: Receiver<Value>,
}
impl Client {
    fn new() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_gdscript-formatter"))
            .arg("lsp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, messages) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            while let Ok(Some(message)) = lsp_server::Message::read(&mut reader) {
                if sender.send(serde_json::to_value(message).unwrap()).is_err() {
                    break;
                }
            }
        });
        let mut client = Self {
            child,
            input,
            messages,
        };
        client.send(
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}),
        );
        let initialized = client.read();
        assert_eq!(initialized["id"], 1);
        assert_eq!(
            initialized["result"]["capabilities"]["textDocumentSync"]["change"],
            1
        );
        client.notify("initialized", json!({}));
        client
    }
    fn send(&mut self, value: Value) {
        let text = value.to_string();
        write!(self.input, "Content-Length: {}\r\n\r\n{}", text.len(), text).unwrap();
        self.input.flush().unwrap();
    }
    fn notify(&mut self, method: &str, params: Value) {
        self.send(json!({"jsonrpc":"2.0","method":method,"params":params}));
    }
    fn read(&self) -> Value {
        self.messages
            .recv_timeout(Duration::from_secs(5))
            .expect("LSP message within 5 s")
    }
    fn open(&mut self, uri: &str, source: &str) -> Value {
        self.notify(
            "textDocument/didOpen",
            json!({"textDocument":{"uri":uri,"languageId":"gdscript","version":1,"text":source}}),
        );
        let message = self.read();
        assert_eq!(message["method"], "textDocument/publishDiagnostics");
        message["params"].clone()
    }
    fn shutdown(&mut self) {
        self.send(json!({"jsonrpc":"2.0","id":99,"method":"shutdown","params":null}));
        assert_eq!(self.read()["id"], 99);
        self.notify("exit", json!(null));
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            assert!(Instant::now() < deadline, "LSP shutdown timed out");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
struct Project {
    root: PathBuf,
}
impl Project {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("gdguard-lsp-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("rules")).unwrap();
        std::fs::write(root.join("project.godot"), "config_version=5\n").unwrap();
        std::fs::write(
            root.join("gdguard.toml"),
            "[custom_rules]\npaths=['rules']\n",
        )
        .unwrap();
        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/rules/prefer-is-empty.yml"),
            root.join("rules/rule.yml"),
        )
        .unwrap();
        Self { root }
    }
    fn uri(&self) -> String {
        format!(
            "file://{}",
            self.root
                .join("main.gd")
                .to_string_lossy()
                .replace('\\', "/")
        )
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn protocol_round_trip_debounce_fixes_config_reload_and_shutdown() {
    let project = Project::new("flow");
    let uri = project.uri();
    let source = "class_name Main\nfunc f(items):\n\tif len(items) == 0:\n\t\tpass\n";
    std::fs::write(project.root.join("main.gd"), source).unwrap();
    let mut client = Client::new();
    let diagnostics = client.open(&uri, source);
    assert!(
        diagnostics["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["code"] == "prefer-is-empty")
    );
    client.send(json!({"jsonrpc":"2.0","id":2,"method":"textDocument/codeAction","params":{"textDocument":{"uri":uri},"range":{"start":{"line":0,"character":0},"end":{"line":4,"character":0}},"context":{"diagnostics":diagnostics["diagnostics"]}}}));
    let response = client.read();
    assert_eq!(response["id"], 2);
    let actions = response["result"].as_array().unwrap();
    assert!(actions.iter().any(|action| {
        action["title"]
            .as_str()
            .unwrap()
            .starts_with("Ignore prefer-is-empty")
    }));
    let action = actions
        .iter()
        .find(|action| action["title"] == "Fix all safe problems in file")
        .unwrap();
    assert_eq!(
        action["edit"]["documentChanges"][0]["textDocument"]["version"],
        1
    );
    let fixed = action["edit"]["documentChanges"][0]["edits"][0]["newText"]
        .as_str()
        .unwrap();
    assert!(fixed.contains("items.is_empty()"));
    client.notify(
        "textDocument/didChange",
        json!({"textDocument":{"uri":uri,"version":2},"contentChanges":[{"text":fixed}]}),
    );
    assert!(
        client
            .messages
            .recv_timeout(Duration::from_millis(60))
            .is_err()
    );
    let after = client.read();
    assert_eq!(after["params"]["version"], 2);
    assert!(
        after["params"]["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    std::fs::write(
        project.root.join("gdguard.toml"),
        "[rules]\nprefer-is-empty='off'\n[custom_rules]\npaths=['rules']\n",
    )
    .unwrap();
    client.notify(
        "textDocument/didChange",
        json!({"textDocument":{"uri":uri,"version":3},"contentChanges":[{"text":source}]}),
    );
    client.read();
    client.notify("workspace/didChangeWatchedFiles",json!({"changes":[{"uri":format!("file://{}",project.root.join("gdguard.toml").display()),"type":2}]}));
    let changed = client.read();
    assert!(
        changed["params"]["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    std::fs::write(
        project.root.join("gdguard.toml"),
        "[rules]\nunknown-typo='off'\n",
    )
    .unwrap();
    client.notify("workspace/didChangeConfiguration", json!({"settings":{}}));
    assert_eq!(
        client.read()["params"]["diagnostics"][0]["code"],
        "config-error"
    );
    client.send(json!({"jsonrpc":"2.0","id":5,"method":"unknown/request","params":{}}));
    assert_eq!(client.read()["error"]["code"], -32601);
    client.notify("textDocument/didClose", json!({"textDocument":{"uri":uri}}));
    assert_eq!(client.read()["params"]["diagnostics"], json!([]));
    client.shutdown();
}

#[test]
fn utf16_positions_round_trip_emoji_cjk_and_bad_offsets() {
    let source = "var text = '😀漢字'\r\nprint(text)\n";
    for (byte, _) in source.char_indices() {
        if source.as_bytes()[byte] != b'\r' && source.as_bytes()[byte] != b'\n' {
            assert_eq!(
                position_to_byte(source, byte_to_position(source, byte)),
                byte
            );
        }
    }
    let emoji = source.find('😀').unwrap();
    assert_eq!(byte_to_position(source, emoji + 4).character, 14);
    assert_eq!(
        position_to_byte(source, lsp_types::Position::new(0, 13)),
        emoji
    );
    assert_eq!(
        position_to_byte(source, lsp_types::Position::new(999, 999)),
        source.len()
    );
    assert_eq!(
        gdguard::lsp::uri_to_path("file:///tmp/a%20%E6%BC%A2.gd").unwrap(),
        PathBuf::from("/tmp/a 漢.gd")
    );
}

#[test]
fn lsp_diagnostics_match_library_results_on_every_sample_script() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/e2e/sample_project");
    let mut paths = Vec::new();
    fn visit(root: &Path, paths: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                if path.file_name().unwrap() != ".gdguard" {
                    visit(&path, paths);
                }
            } else if path.extension().is_some_and(|extension| extension == "gd") {
                paths.push(path);
            }
        }
    }
    visit(&root, &mut paths);
    paths.sort();
    let mut client = Client::new();
    for path in paths {
        let source = std::fs::read_to_string(&path).unwrap();
        let uri = format!("file://{}", path.to_string_lossy().replace('\\', "/"));
        let actual = client.open(&uri, &source);
        let config = gdguard::linter::api::load_config_for(&path).unwrap();
        let lint = gdguard::linter::api::lint_source(&path, &source, &config).unwrap();
        let mut expected = Vec::new();
        for diagnostic in lint.diagnostics {
            expected.push(gdguard::lsp::diagnostic_json(&source, &diagnostic));
        }
        assert_eq!(actual["diagnostics"], json!(expected), "{}", path.display());
    }
    client.shutdown();
}
