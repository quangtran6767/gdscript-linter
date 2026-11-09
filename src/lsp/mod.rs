//! Synchronous stdio LSP server, sharing exactly the CLI's library pipeline.
use crate::linter::api;
use crate::linter::diagnostic::{Diagnostic, FixSafety, Span};
use lsp_server::{Connection, Message, Notification, Request, Response};
use lsp_types::{Position, Range};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const DEBOUNCE: Duration = Duration::from_millis(150);
struct Document {
    path: PathBuf,
    source: String,
    version: i64,
    pending: Option<Instant>,
}

pub fn run_stdio() -> Result<(), String> {
    let (connection, threads) = Connection::stdio();
    let result = serve(&connection);
    drop(connection);
    threads.join().map_err(|error| error.to_string())?;
    result
}

/// Public for in-memory protocol tests; production uses stdio.
pub fn serve(connection: &Connection) -> Result<(), String> {
    let initialization=connection.initialize(json!({
        "positionEncoding":"utf-16","textDocumentSync":{"openClose":true,"change":1,"save":{"includeText":true}},
        "codeActionProvider":{"codeActionKinds":["quickfix","source.fixAll.gdguard"]}
    })).map_err(|error|format!("initialize: {}",error))?;
    let dynamic_watch = initialization
        .pointer("/capabilities/workspace/didChangeWatchedFiles/dynamicRegistration")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if dynamic_watch {
        let request = Request::new(
            "gdguard-watch".to_string().into(),
            "client/registerCapability".to_string(),
            json!({"registrations":[{
            "id":"gdguard-config-rules","method":"workspace/didChangeWatchedFiles","registerOptions":{"watchers":[
                {"globPattern":"**/gdguard.toml"},{"globPattern":"**/project.godot"},{"globPattern":"**/.editorconfig"},
                {"globPattern":"**/*.{js,yml,yaml,gd,tscn}"}
            ]}}]}),
        );
        send(connection, request.into())?;
    }
    let mut documents: BTreeMap<String, Document> = BTreeMap::new();
    let mut shutdown = false;
    loop {
        match connection.receiver.recv_timeout(Duration::from_millis(20)) {
            Ok(Message::Request(request)) => {
                if request.method == "shutdown" {
                    shutdown = true;
                    send(connection, Response::new_ok(request.id, Value::Null).into())?;
                    continue;
                }
                let response = if shutdown {
                    Response::new_err(request.id, -32600, "Server is shutting down".to_string())
                } else {
                    match handle_request(&request, &documents) {
                        Ok(value) => Response::new_ok(request.id, value),
                        Err((code, message)) => {
                            eprintln!("gdguard LSP: {}", message);
                            Response::new_err(request.id, code, message)
                        }
                    }
                };
                send(connection, response.into())?;
            }
            Ok(Message::Notification(notification)) => {
                if notification.method == "exit" {
                    return if shutdown {
                        Ok(())
                    } else {
                        Err("exit received before shutdown".to_string())
                    };
                }
                if !shutdown {
                    if let Err(error) =
                        handle_notification(connection, &notification, &mut documents)
                    {
                        eprintln!("gdguard LSP: {}", error);
                    }
                }
            }
            Err(error) if error.is_disconnected() => return Ok(()),
            Ok(Message::Response(_)) | Err(_) => {}
        }
        if !shutdown {
            for (uri, document) in &mut documents {
                if document
                    .pending
                    .is_some_and(|deadline| Instant::now() >= deadline)
                {
                    document.pending = None;
                    publish(connection, uri, document)?;
                }
            }
        }
    }
}

fn send(connection: &Connection, message: Message) -> Result<(), String> {
    connection
        .sender
        .send(message)
        .map_err(|error| error.to_string())
}
fn field_string<'a>(value: &'a Value, pointer: &str) -> Result<&'a str, String> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing string {}", pointer))
}

fn handle_notification(
    connection: &Connection,
    notification: &Notification,
    documents: &mut BTreeMap<String, Document>,
) -> Result<(), String> {
    let params = &notification.params;
    match notification.method.as_str() {
        "textDocument/didOpen" => {
            let uri = field_string(params, "/textDocument/uri")?.to_string();
            let document = Document {
                path: uri_to_path(&uri)?,
                source: field_string(params, "/textDocument/text")?.to_string(),
                version: params
                    .pointer("/textDocument/version")
                    .and_then(Value::as_i64)
                    .unwrap_or(0),
                pending: None,
            };
            publish(connection, &uri, &document)?;
            documents.insert(uri, document);
        }
        "textDocument/didChange" => {
            let uri = field_string(params, "/textDocument/uri")?;
            let document = documents
                .get_mut(uri)
                .ok_or("change for an unopened document")?;
            let version = params
                .pointer("/textDocument/version")
                .and_then(Value::as_i64)
                .ok_or("missing version")?;
            if version <= document.version {
                return Ok(());
            }
            let changes = params["contentChanges"]
                .as_array()
                .ok_or("missing contentChanges")?;
            let mut updated = None;
            for change in changes {
                if change.get("range").is_some() {
                    return Err(
                        "full document text is required (textDocumentSync=Full)".to_string()
                    );
                }
                updated = Some(
                    change["text"]
                        .as_str()
                        .ok_or("missing change text")?
                        .to_string(),
                );
            }
            if let Some(source) = updated {
                document.source = source;
                document.version = version;
                document.pending = Some(Instant::now() + DEBOUNCE);
            }
        }
        "textDocument/didSave" => {
            let uri = field_string(params, "/textDocument/uri")?;
            if let Some(document) = documents.get_mut(uri) {
                if let Some(text) = params["text"].as_str() {
                    document.source = text.to_string();
                }
                document.pending = None;
                publish(connection, uri, document)?;
            }
        }
        "textDocument/didClose" => {
            let uri = field_string(params, "/textDocument/uri")?;
            documents.remove(uri);
            send(
                connection,
                Notification::new(
                    "textDocument/publishDiagnostics".to_string(),
                    json!({"uri":uri,"diagnostics":[]}),
                )
                .into(),
            )?;
        }
        "workspace/didChangeWatchedFiles" | "workspace/didChangeConfiguration" => {
            for (uri, document) in documents {
                document.pending = None;
                publish(connection, uri, document)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn publish(connection: &Connection, uri: &str, document: &Document) -> Result<(), String> {
    let diagnostics = match api::load_config_for(&document.path)
        .map_err(|error| error.to_string())
        .and_then(|config| api::lint_source(&document.path, &document.source, &config))
    {
        Ok(result) => {
            let mut diagnostics = Vec::with_capacity(result.diagnostics.len());
            for diagnostic in result.diagnostics {
                diagnostics.push(diagnostic_json(&document.source, &diagnostic));
            }
            diagnostics
        }
        Err(error) => {
            eprintln!("gdguard LSP: {}", error);
            vec![
                json!({"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":0}},"severity":1,"source":"gdguard","code":"config-error","message":error}),
            ]
        }
    };
    send(
        connection,
        Notification::new(
            "textDocument/publishDiagnostics".to_string(),
            json!({"uri":uri,"version":document.version,"diagnostics":diagnostics}),
        )
        .into(),
    )
}

pub fn diagnostic_json(source: &str, diagnostic: &Diagnostic) -> Value {
    let message = match &diagnostic.help {
        Some(help) => format!("{}\nHelp: {}", diagnostic.message, help),
        None => diagnostic.message.clone(),
    };
    json!({"range":span_range(source,diagnostic.span),"severity":match diagnostic.severity {crate::linter::diagnostic::Severity::Error=>1,crate::linter::diagnostic::Severity::Warning=>2,crate::linter::diagnostic::Severity::Info=>3},
        "code":diagnostic.rule,"source":"gdguard","message":message,
        "codeDescription":{"href":"https://github.com/quangtran6767/gdscript-linter/blob/main/docs/configuration.md"}})
}

fn handle_request(
    request: &Request,
    documents: &BTreeMap<String, Document>,
) -> Result<Value, (i32, String)> {
    if request.method != "textDocument/codeAction" {
        return Err((-32601, format!("unsupported method {}", request.method)));
    }
    let uri = field_string(&request.params, "/textDocument/uri").map_err(invalid_params)?;
    let document = documents
        .get(uri)
        .ok_or_else(|| invalid_params("document is not open".to_string()))?;
    let range: Range = serde_json::from_value(request.params["range"].clone())
        .map_err(|error| invalid_params(error.to_string()))?;
    let config =
        api::load_config_for(&document.path).map_err(|error| (-32603, error.to_string()))?;
    let lint = api::lint_source(&document.path, &document.source, &config)
        .map_err(|error| (-32603, error))?;
    let start = position_to_byte(&document.source, range.start);
    let end = position_to_byte(&document.source, range.end);
    let mut actions = Vec::new();
    for diagnostic in lint.diagnostics {
        if diagnostic.span.end_byte < start || diagnostic.span.start_byte > end {
            continue;
        }
        for fix in &diagnostic.fixes {
            let mut edits = Vec::with_capacity(fix.edits.len());
            for edit in &fix.edits {
                edits.push(json!({"range":span_range(&document.source,edit.span),"newText":edit.replacement}));
            }
            actions.push(json!({"title":format!("Fix: {} ({})",fix.description,fix.safety.as_str()),"kind":"quickfix","isPreferred":fix.safety==FixSafety::Safe,
                "diagnostics":[diagnostic_json(&document.source,&diagnostic)],"edit":versioned_edit(uri,document.version,&edits)}));
        }
        let line_start = document.source[..diagnostic.span.start_byte]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        let line = &document.source[line_start..];
        let indentation = &line[..line.len() - line.trim_start_matches([' ', '\t']).len()];
        let newline = if document.source.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let position = byte_to_position(&document.source, line_start);
        let edit = json!({"range":{"start":position,"end":position},"newText":format!("{}# gdguard-ignore-next-line {} -- TODO: explain this exception{}",indentation,diagnostic.rule,newline)});
        actions.push(json!({"title":format!("Ignore {} on this line (add reason)",diagnostic.rule),"kind":"quickfix","edit":versioned_edit(uri,document.version,&[edit])}));
    }
    let fixed = api::fix_source(&document.path, &document.source, &config, false)
        .map_err(|error| (-32603, error))?;
    if fixed.output != document.source {
        let range = span_range(
            &document.source,
            Span::from_byte_range(&document.source, 0, document.source.len()),
        );
        actions.push(json!({"title":"Fix all safe problems in file","kind":"source.fixAll.gdguard","edit":versioned_edit(uri,document.version,&[json!({"range":range,"newText":fixed.output})])}));
    }
    if let Some(only) = request
        .params
        .pointer("/context/only")
        .and_then(Value::as_array)
    {
        actions.retain(|action| {
            only.iter().any(|kind| {
                kind.as_str().is_some_and(|kind| {
                    action["kind"].as_str().is_some_and(|actual| {
                        actual == kind || actual.starts_with(&format!("{}.", kind))
                    })
                })
            })
        });
    }
    Ok(json!(actions))
}
fn invalid_params(message: String) -> (i32, String) {
    (-32602, message)
}
fn versioned_edit(uri: &str, version: i64, edits: &[Value]) -> Value {
    json!({"documentChanges":[{"textDocument":{"uri":uri,"version":version},"edits":edits}]})
}
fn span_range(source: &str, span: Span) -> Range {
    Range::new(
        byte_to_position(source, span.start_byte),
        byte_to_position(source, span.end_byte),
    )
}

pub fn byte_to_position(source: &str, byte: usize) -> Position {
    let mut byte = byte.min(source.len());
    while !source.is_char_boundary(byte) {
        byte -= 1;
    }
    let prefix = &source[..byte];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count();
    let start = prefix.rfind('\n').map_or(0, |index| index + 1);
    Position::new(
        line as u32,
        source[start..byte].encode_utf16().count() as u32,
    )
}
pub fn position_to_byte(source: &str, position: Position) -> usize {
    let mut start = 0;
    for (line, text) in source.split_inclusive('\n').enumerate() {
        if line == position.line as usize {
            let text = text.trim_end_matches(['\r', '\n']);
            let mut units = 0;
            for (offset, character) in text.char_indices() {
                if units + character.len_utf16() > position.character as usize {
                    return start + offset;
                }
                units += character.len_utf16();
                if units == position.character as usize {
                    return start + offset + character.len_utf8();
                }
            }
            return start + text.len();
        }
        start += text.len();
    }
    source.len()
}

pub fn uri_to_path(uri: &str) -> Result<PathBuf, String> {
    let rest = uri
        .strip_prefix("file://")
        .ok_or("only file:// document URIs are supported")?;
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    if !rest.starts_with('/') {
        return Err("file URI must have an absolute local path".to_string());
    }
    let bytes = rest.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let pair = bytes
                .get(index + 1..index + 3)
                .ok_or("incomplete percent escape")?;
            let text = std::str::from_utf8(pair).map_err(|error| error.to_string())?;
            decoded.push(u8::from_str_radix(text, 16).map_err(|error| error.to_string())?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    let path = String::from_utf8(decoded).map_err(|error| error.to_string())?;
    #[cfg(windows)]
    let path = if path.as_bytes().get(2) == Some(&b':') {
        path[1..].to_string()
    } else {
        path
    };
    Ok(Path::new(&path).to_path_buf())
}
