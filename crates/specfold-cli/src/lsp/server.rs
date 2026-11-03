//! A minimal language server over stdio: Content-Length framed JSON-RPC 2.0, full text sync,
//! diagnostics only. Each open, change or save puts the buffer in the overlay and checks the file.

use super::actions::{RUN, choices, code_actions, runnable, split_words};
use super::wire::{read_message, uri_path, utf16, write_message};
use crate::cli::args::global::Global;
use crate::commands::session::Location;
use crate::commands::verify::findings;
use crate::output::ci;
use serde_json::{Value, json};
use specfold_core::check::finding::visible;
use specfold_core::check::{Finding, Fix, Severity};
use specfold_core::corpus::overlay;
use std::path::{Path, PathBuf};

/// The range of the one character a finding names by column (H1), in UTF-16 units.
fn char_range(f: &Finding, line_text: Option<&str>) -> Option<Value> {
    if f.rule != "H1" {
        return None;
    }
    let col: usize = f.message.rsplit("at column ").next()?.trim().parse().ok()?;
    let l = line_text?;
    let (b, c) = l.char_indices().nth(col.checked_sub(1)?)?;
    let line = f.line.max(1) - 1;
    Some(json!({
        "start": {"line": line, "character": utf16(l, b)},
        "end": {"line": line, "character": utf16(l, b + c.len_utf8())},
    }))
}

/// Size and modification time of every file under `root` (skipping `.git` but for HEAD and
/// index, and the specfold cache which checks write themselves), hashed: any change on disk that
/// a check could see changes the stamp.
fn disk_stamp(root: &Path) -> u64 {
    let cache = specfold_core::config::paths::cache_dir().map(|c| c.canonicalize().unwrap_or(c));
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let mut stack = vec![root.to_path_buf()];
    let mut seen = vec![];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            let Ok(m) = e.metadata() else { continue };
            if m.is_dir() {
                if p.file_name().is_some_and(|n| n == ".git") {
                    for f in ["HEAD", "index"] {
                        if let Ok(m) = std::fs::metadata(p.join(f)) {
                            seen.push((p.join(f), m.len(), m.modified().ok()));
                        }
                    }
                } else if cache.as_deref() != Some(p.as_path()) {
                    stack.push(p);
                }
            } else {
                seen.push((p, m.len(), m.modified().ok()));
            }
        }
    }
    seen.sort();
    seen.hash(&mut h);
    h.finish()
}

pub struct Server {
    global: Global,
    root: Option<PathBuf>,
    /// The last full check and the disk/buffer stamp it was made at.
    cache: Option<((u64, u64), Vec<Finding>)>,
    /// The process directory: buffer paths are read from it.
    loc: Location,
}

impl Server {
    pub fn new(global: Global) -> Self {
        // No cached configuration: each check reads spec.toml afresh.
        Server { global: Global { loc: None, ..global }, root: None, cache: None, loc: Location::process() }
    }

    fn root(&mut self) -> Option<PathBuf> {
        if self.root.is_none() {
            self.root = crate::session::config(&self.global).ok().map(|c| c.root);
        }
        self.root.clone()
    }

    /// The diagnostics for one document, or an empty list when it is not part of the corpus.
    pub fn diagnostics(&mut self, path: &Path) -> Vec<Value> {
        let Some(root) = self.root() else { return vec![] };
        let abs = self.loc.absolute(path);
        let Some(rel) = specfold_core::paths::rel_of(&root, &abs) else { return vec![] };
        let key = (disk_stamp(&root), overlay::stamp());
        let fresh = std::env::var_os("SPEC_LSP_NO_CACHE").is_some();
        if fresh || self.cache.as_ref().is_none_or(|(k, _)| *k != key) {
            if std::env::var_os("SPEC_LSP_TRACE").is_some() {
                eprintln!("spec lsp: full check");
            }
            let Ok(all) = findings::all_findings(&self.global) else { return vec![] };
            self.cache = Some((key, all));
        }
        let all = self.cache.as_ref().map(|(_, l)| l.as_slice()).unwrap_or_default();
        let text = overlay::read(&root.join(&rel)).unwrap_or_default();
        all.iter()
            .filter(|f| f.path == rel)
            .map(|f| {
                let line = f.line.max(1) - 1;
                let range = char_range(f, text.lines().nth(line)).unwrap_or_else(
                    || json!({"start": {"line": line, "character": 0}, "end": {"line": line + 1, "character": 0}}),
                );
                let mut d = json!({
                    "range": range,
                    "severity": if f.severity == Severity::Error { 1 } else { 2 },
                    "code": f.rule,
                    "source": "spec",
                    "message": ci::message(f).split('\n').map(visible).collect::<Vec<_>>().join("\n"),
                });
                let mut data = json!({"line": f.line});
                if let Some(Fix::Replace { line, from, to, .. }) = &f.fix {
                    data = json!({"line": line, "from": from, "to": to});
                }
                let ch = choices(f);
                if !ch.is_empty() {
                    data["choices"] = ch.iter().map(|(from, to)| json!({"from": from, "to": to})).collect();
                }
                // A pick is offered as edits to the buffer, so not again as a command on disk.
                if let Some(c) = runnable(f).filter(|_| ch.is_empty()) {
                    data["command"] = json!(c);
                }
                if data.as_object().is_some_and(|o| o.len() > 1) {
                    d["data"] = data;
                }
                d
            })
            .collect()
    }

    /// Runs a fix command a current finding offers; anything else is refused.
    fn execute(&mut self, params: &Value, id: &Value) -> Value {
        let fail = |m: String| json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32602, "message": m}});
        let cmd = params.pointer("/arguments/0").and_then(Value::as_str).unwrap_or("");
        if params.get("command").and_then(Value::as_str) != Some(RUN) {
            return fail(format!("unknown command; this server runs only {RUN}"));
        }
        let offered = self.cache.as_ref().is_some_and(|(_, all)| all.iter().any(|f| runnable(f) == Some(cmd)));
        let Some(root) = self.root() else { return fail("no spec directory".into()) };
        if !offered {
            return fail(format!("{cmd:?} is not a fix any current finding offers"));
        }
        let Ok(exe) = std::env::current_exe() else { return fail("cannot find the spec binary".into()) };
        let words = split_words(cmd);
        let dir = self.global.dir.clone().unwrap_or(root);
        match std::process::Command::new(exe).args(&words[1..]).current_dir(dir).output() {
            Ok(o) => {
                self.cache = None;
                let text = String::from_utf8_lossy(&o.stdout).into_owned();
                json!({"jsonrpc": "2.0", "id": id, "result": {"exit": o.status.code(), "output": text}})
            }
            Err(e) => fail(format!("cannot run spec: {e}")),
        }
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
                            "capabilities": {
                                "textDocumentSync": {"openClose": true, "change": 1, "save": {"includeText": true}},
                                "codeActionProvider": {"codeActionKinds": ["quickfix"]},
                                "executeCommandProvider": {"commands": [RUN]}
                            },
                            "serverInfo": {"name": "specfold", "version": env!("CARGO_PKG_VERSION")}
                        }))],
                        false,
                    )
                }
                "shutdown" => (vec![reply(Value::Null)], false),
                "workspace/executeCommand" => (vec![self.execute(&params, &id)], false),
                "textDocument/codeAction" => {
                    (vec![reply(Value::Array(code_actions(&uri, path.as_deref(), &params)))], false)
                }
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
                overlay::clear(&self.loc.absolute(&path));
                let clear = json!({"jsonrpc": "2.0", "method": "textDocument/publishDiagnostics",
                                   "params": {"uri": uri, "diagnostics": []}});
                return (vec![clear], false);
            }
            _ => return (vec![], method == "exit"),
        };
        match text {
            Some(t) => overlay::set(self.loc.absolute(&path), t),
            // A save without text: the disk is current again.
            None if method == "textDocument/didSave" => overlay::clear(&self.loc.absolute(&path)),
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
    loop {
        let msg = match read_message(&mut input) {
            Ok(None) => break,
            Ok(Some(Ok(m))) => m,
            Ok(Some(Err(e))) => {
                let err = json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": format!("parse error: {e}")}});
                write_message(&mut out, &err)?;
                continue;
            }
            Err(e) => {
                eprintln!("spec lsp: stopping: {e}");
                return Err(e);
            }
        };
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
