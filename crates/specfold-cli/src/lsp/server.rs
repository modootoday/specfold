//! A minimal language server over stdio: Content-Length framed JSON-RPC 2.0, full text sync,
//! diagnostics only. Each open, change or save puts the buffer in the overlay and checks the file.

use crate::cli::args::global::Global;
use crate::commands::verify::check;
use crate::output::ci;
use serde_json::{Value, json};
use specfold_core::check::Severity;
use specfold_core::corpus::overlay;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

/// One framed message, or None at the end of input.
pub fn read_message(r: &mut impl BufRead) -> std::io::Result<Option<Value>> {
    let mut len = None;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let line = line.trim_end();
        if line.is_empty() {
            if len.is_some() {
                break;
            }
            continue;
        }
        if let Some((k, v)) = line.split_once(':')
            && k.eq_ignore_ascii_case("content-length")
        {
            len = v.trim().parse::<usize>().ok();
        }
    }
    let mut body = vec![0; len.unwrap_or(0)];
    r.read_exact(&mut body)?;
    Ok(Some(serde_json::from_slice(&body).unwrap_or(Value::Null)))
}

pub fn write_message(w: &mut impl Write, v: &Value) -> std::io::Result<()> {
    let body = v.to_string();
    write!(w, "Content-Length: {}\r\n\r\n{body}", body.len())?;
    w.flush()
}

/// `file:///a%20b/c.md` → `/a b/c.md`. Other schemes are not files.
pub fn uri_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let b = rest.as_bytes();
    let mut out = vec![];
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Ok(n) = u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or(""), 16)
        {
            out.push(n);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    let s = String::from_utf8(out).ok()?;
    // Windows: file:///C:/x → C:/x
    let s = if s.len() > 2 && s.as_bytes()[0] == b'/' && s.as_bytes()[2] == b':' { s[1..].to_string() } else { s };
    Some(PathBuf::from(s))
}

pub struct Server {
    global: Global,
    root: Option<PathBuf>,
}

impl Server {
    pub fn new(global: Global) -> Self {
        Server { global, root: None }
    }

    fn root(&mut self) -> Option<PathBuf> {
        if self.root.is_none() {
            self.root = crate::session::config(&self.global).ok().map(|c| c.root.clone());
        }
        self.root.clone()
    }

    /// The diagnostics for one document, or an empty list when it is not part of the corpus.
    pub fn diagnostics(&mut self, path: &Path) -> Vec<Value> {
        let Some(root) = self.root() else { return vec![] };
        let abs = check::absolute(path);
        let Ok(rel) = abs.strip_prefix(&root) else { return vec![] };
        let rel = rel.to_string_lossy().replace('\\', "/");
        let Ok(list) = check::file_findings(&self.global, &rel) else { return vec![] };
        list.iter()
            .map(|f| {
                let line = f.line.max(1) - 1;
                json!({
                    "range": {"start": {"line": line, "character": 0}, "end": {"line": line + 1, "character": 0}},
                    "severity": if f.severity == Severity::Error { 1 } else { 2 },
                    "code": f.rule,
                    "source": "spec",
                    "message": ci::message(f),
                })
            })
            .collect()
    }

    fn publish(&mut self, uri: &str, path: &Path) -> Value {
        let diagnostics = self.diagnostics(path);
        json!({"jsonrpc": "2.0", "method": "textDocument/publishDiagnostics",
               "params": {"uri": uri, "diagnostics": diagnostics}})
    }

    /// Handles one message; returns what to send back and whether to stop.
    pub fn handle(&mut self, msg: &Value) -> (Vec<Value>, bool) {
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        let doc = params.get("textDocument");
        let uri = doc.and_then(|d| d.get("uri")).and_then(Value::as_str).unwrap_or("").to_string();
        let path = uri_path(&uri);
        if let Some(id) = msg.get("id").cloned() {
            let reply = |result: Value| json!({"jsonrpc": "2.0", "id": id, "result": result});
            return match method {
                "initialize" => {
                    if self.global.dir.is_none()
                        && let Some(p) = params.get("rootUri").and_then(Value::as_str).and_then(uri_path)
                    {
                        self.global.dir = Some(p);
                    }
                    (
                        vec![reply(json!({
                            "capabilities": {"textDocumentSync": {"openClose": true, "change": 1, "save": {"includeText": true}}},
                            "serverInfo": {"name": "specfold", "version": env!("CARGO_PKG_VERSION")}
                        }))],
                        false,
                    )
                }
                "shutdown" => (vec![reply(Value::Null)], false),
                other => (
                    vec![json!({"jsonrpc": "2.0", "id": id,
                                "error": {"code": -32601, "message": format!("method not found: {other}")}})],
                    false,
                ),
            };
        }
        let Some(path) = path else { return (vec![], method == "exit") };
        let text = match method {
            "textDocument/didOpen" => doc.and_then(|d| d.get("text")).and_then(Value::as_str).map(str::to_string),
            "textDocument/didChange" => params
                .get("contentChanges")
                .and_then(Value::as_array)
                .and_then(|c| c.last())
                .and_then(|c| c.get("text"))
                .and_then(Value::as_str)
                .map(str::to_string),
            "textDocument/didSave" => params.get("text").and_then(Value::as_str).map(str::to_string),
            "textDocument/didClose" => {
                overlay::clear(&check::absolute(&path));
                let clear = json!({"jsonrpc": "2.0", "method": "textDocument/publishDiagnostics",
                                   "params": {"uri": uri, "diagnostics": []}});
                return (vec![clear], false);
            }
            _ => return (vec![], method == "exit"),
        };
        match text {
            Some(t) => overlay::set(check::absolute(&path), t),
            // A save without text: the disk is current again.
            None if method == "textDocument/didSave" => overlay::clear(&check::absolute(&path)),
            None => {}
        }
        (vec![self.publish(&uri, &path)], false)
    }
}

pub fn serve(global: Global) -> std::io::Result<()> {
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let mut out = std::io::stdout();
    let mut server = Server::new(global);
    while let Some(msg) = read_message(&mut input)? {
        let (replies, stop) = server.handle(&msg);
        for r in &replies {
            write_message(&mut out, r)?;
        }
        if stop {
            break;
        }
    }
    Ok(())
}
