//! A minimal language server over stdio: Content-Length framed JSON-RPC 2.0, full text sync,
//! diagnostics only. Each open, change or save puts the buffer in the overlay and checks the file.

use crate::cli::args::global::Global;
use crate::commands::verify::check;
use crate::output::ci;
use serde_json::{Value, json};
use specfold_core::check::finding::visible;
use specfold_core::check::{Finding, Fix, Severity};
use specfold_core::corpus::overlay;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

/// Largest message body accepted; a bigger Content-Length is treated as a broken stream.
const MAX_BODY: usize = 64 << 20;

fn broken(why: String) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, why)
}

/// One framed message (`Err` holds a JSON parse error for a -32700 reply), or None at the end
/// of input. A frame that cannot be delimited is an I/O error naming why.
pub fn read_message(r: &mut impl BufRead) -> std::io::Result<Option<Result<Value, String>>> {
    let mut len = None;
    let mut headers = false;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line)? == 0 {
            if headers {
                return Err(broken("input ended inside message headers".into()));
            }
            return Ok(None);
        }
        let line = line.trim_end();
        if line.is_empty() {
            if headers {
                break;
            }
            continue;
        }
        headers = true;
        if let Some((k, v)) = line.split_once(':')
            && k.eq_ignore_ascii_case("content-length")
        {
            len = Some(v.trim().parse::<usize>().map_err(|_| broken(format!("bad Content-Length header: {line}")))?);
        }
    }
    let len = len.ok_or_else(|| broken("message headers have no Content-Length".into()))?;
    if len > MAX_BODY {
        return Err(broken(format!("Content-Length {len} exceeds the {MAX_BODY}-byte limit")));
    }
    let mut body = vec![0; len];
    r.read_exact(&mut body).map_err(|_| broken(format!("input ended inside a {len}-byte message body")))?;
    Ok(Some(serde_json::from_slice(&body).map_err(|e| e.to_string())))
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

/// UTF-16 code units in `s` up to byte `b`: LSP positions count UTF-16 by default.
fn utf16(s: &str, b: usize) -> usize {
    s[..b].encode_utf16().count()
}

/// One quickfix per diagnostic that carries a replacement, edited against the buffer text.
pub fn code_actions(uri: &str, path: Option<&Path>, params: &Value) -> Vec<Value> {
    let Some(path) = path else { return vec![] };
    let Ok(text) = overlay::read(&check::absolute(path)) else { return vec![] };
    let diags = params.pointer("/context/diagnostics").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut out = vec![];
    for d in &diags {
        let Some(data) = d.get("data") else { continue };
        let line = data.get("line").and_then(Value::as_u64).unwrap_or(0) as usize;
        let edit = |from: &str, to: &str| -> Option<Value> {
            let l = text.lines().nth(line.checked_sub(1)?)?;
            let b = l.find(from)?;
            let (start, end) = (utf16(l, b), utf16(l, b + from.len()));
            let range =
                json!({"start": {"line": line - 1, "character": start}, "end": {"line": line - 1, "character": end}});
            Some(json!({uri: [{"range": range, "newText": to}]}))
        };
        if let (Some(from), Some(to)) =
            (data.get("from").and_then(Value::as_str), data.get("to").and_then(Value::as_str))
            && let Some(changes) = edit(from, to)
        {
            out.push(json!({
                "title": if to.is_empty() {
                    format!("spec: remove `{}`", visible(from))
                } else {
                    format!("spec: replace `{}` with `{}`", visible(from), visible(to))
                },
                "kind": "quickfix",
                "diagnostics": [d],
                "isPreferred": true,
                "edit": {"changes": changes},
            }));
        }
        for c in data.get("choices").and_then(Value::as_array).into_iter().flatten() {
            let (Some(from), Some(to)) = (c.get("from").and_then(Value::as_str), c.get("to").and_then(Value::as_str))
            else {
                continue;
            };
            if let Some(changes) = edit(from, to) {
                out.push(json!({
                    "title": format!("spec: use `{}`", visible(to)),
                    "kind": "quickfix",
                    "diagnostics": [d],
                    "edit": {"changes": changes},
                }));
            }
        }
        if let Some(cmd) = data.get("command").and_then(Value::as_str) {
            out.push(json!({
                "title": format!("spec: run `{cmd}`"),
                "kind": "quickfix",
                "diagnostics": [d],
                "command": {"title": format!("run `{cmd}`"), "command": RUN, "arguments": [cmd]},
            }));
        }
    }
    out
}

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

/// The one command the server executes: a finding's own `--write` fix command.
const RUN: &str = "spec.run";

/// What a quick fix may change for a finding: one replacement per candidate it lists, written
/// the way `spec fix --pick` would.
fn choices(f: &Finding) -> Vec<(String, String)> {
    let each = |from: &str, to: &dyn Fn(&str) -> String| -> Vec<(String, String)> {
        f.candidates.iter().map(|c| (from.to_string(), to(c))).collect()
    };
    match f.rule.as_str() {
        "L1" if f.message.contains("]] is ambiguous") => {
            let Some(target) = f.message.split("]]").next().and_then(|s| s.strip_prefix("[[")) else { return vec![] };
            let base = target.split('#').next().unwrap_or(target);
            each(&format!("[[{base}"), &|c| format!("[[{c}"))
        }
        "L1" => match f.message.split_once(" has no anchor #") {
            Some((_, a)) => each(&format!("#{a}"), &|c| format!("#{c}")),
            None => vec![],
        },
        "L2" => match f.message.strip_prefix("cites the path ") {
            Some(p) => each(p.split(", which is not a document").next().unwrap_or(p), &|c| format!("[[{c}]]")),
            None => vec![],
        },
        "S4" => match f.message.strip_prefix("status '").and_then(|r| r.split_once("' is not allowed")) {
            // The value itself, unless it also occurs inside the key on that line.
            Some((status, _)) if !"status".contains(status) => each(status, &|c| c.to_string()),
            _ => vec![],
        },
        _ => vec![],
    }
}

/// A fix command the editor may run for the user: it writes with `--write` and needs nothing
/// filled in.
fn runnable(f: &Finding) -> Option<&str> {
    match &f.fix {
        Some(Fix::Cmd(c)) if c.starts_with("spec ") && c.contains(" --write") && !c.contains('…') => Some(c),
        _ => None,
    }
}

/// Splits a command the way a POSIX shell would for the quoting `spec` itself writes.
fn split_words(s: &str) -> Vec<String> {
    let (mut out, mut cur, mut quoted, mut any) = (vec![], String::new(), false, false);
    for ch in s.chars() {
        match ch {
            '\'' => {
                quoted = !quoted;
                any = true;
            }
            ' ' if !quoted => {
                if any || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                any = false;
            }
            '\\' if !quoted => {}
            c => cur.push(c),
        }
    }
    if any || !cur.is_empty() {
        out.push(cur);
    }
    out
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
}

impl Server {
    pub fn new(global: Global) -> Self {
        Server { global, root: None, cache: None }
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
        let key = (disk_stamp(&root), overlay::stamp());
        let fresh = std::env::var_os("SPEC_LSP_NO_CACHE").is_some();
        if fresh || self.cache.as_ref().is_none_or(|(k, _)| *k != key) {
            if std::env::var_os("SPEC_LSP_TRACE").is_some() {
                eprintln!("spec lsp: full check");
            }
            let Ok(all) = check::all_findings(&self.global) else { return vec![] };
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
                    "message": visible(&ci::message(f)),
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

#[cfg(test)]
mod tests {
    use super::*;
    use specfold_core::check::Severity;

    fn finding(rule: &str, message: &str, candidates: &[&str]) -> Finding {
        Finding::new(rule, Severity::Error, "a.md", 1, message)
            .with_candidates(candidates.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn typos_get_one_replacement_per_candidate() {
        let f = finding("S4", "status 'actve' is not allowed for adr (allowed: draft, active)", &["active", "draft"]);
        assert_eq!(choices(&f)[0], ("actve".to_string(), "active".to_string()));
        let f = finding("L2", "cites the path a/b.md, which is not a document (no such file)", &["adr:x/1"]);
        assert_eq!(choices(&f), vec![("a/b.md".to_string(), "[[adr:x/1]]".to_string())]);
        assert!(choices(&finding("S4", "status 'tat' is not allowed for adr", &["draft"])).is_empty());
    }
}
