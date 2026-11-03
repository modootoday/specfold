//! `spec doctor`: problems that stop the other commands from working well, found without writing
//! anything. Rules judge documents; doctor judges the setup around them: configuration,
//! conflict markers, ignored local documents, duplicate ids and half-finished moves.

use crate::cli::args::global::Global;
use crate::output::{Code, Output};
use crate::session;
use anyhow::Result;
use serde_json::json;
use specfold_core::graph::Resolution;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const SKIP: [&str; 4] = [".git", "target", "node_modules", ".cargo-target"];

/// The problems found so far, as output items.
struct Problems {
    o: Output,
    found: usize,
}

impl Problems {
    fn add(&mut self, check: &str, path: &str, msg: String, fix: &str) {
        self.found += 1;
        let o = std::mem::replace(&mut self.o, Output::new("doctor"));
        self.o = o.item(
            json!({"check": check, "path": path, "message": msg, "fix": fix}),
            format!("{check:<9} {path}  {msg}\n          fix: {fix}"),
        );
    }
}

pub fn run(g: &Global) -> Result<Output> {
    let mut p = Problems { o: Output::new("doctor"), found: 0 };
    session::hold_config_warnings();
    let cfg = match session::config(g) {
        Ok(c) => c,
        Err(e) => {
            // C235: spec init cannot read the file either; the fix is an edit at the line TOML names.
            p.add("config", "spec.toml", format!("{e:#}"), &config_fix(&format!("{e:#}")));
            return Ok(p.o.code(Code::Findings).next("spec explain config"));
        }
    };
    let root = cfg.spec_home.clone();
    let no_config = !root.join("spec.toml").exists();
    if no_config {
        p.add("config", "spec.toml", "no spec.toml here; spec runs on defaults".into(), "spec init --write");
    }
    for w in session::config_warnings(&cfg) {
        let next = warning_fix(&w);
        p.add("config", "spec.toml", w, next);
    }
    let mut files = vec![];
    walk(&root, &mut files);
    conflicts(&root, &files, &mut p);
    local_ignored(&root, &mut p);
    corpus_problems(g, &mut p);
    let o = remotes(&cfg, p.o);
    let o = o.head(format!("{} spec files read", files.len()));
    Ok(if p.found == 0 {
        o.foot("no setup problems found").next("spec check")
    } else {
        if no_config { o.code(Code::Findings).next("spec init --write") } else { o.code(Code::Findings) }
    })
}

/// Merge conflict markers left in spec files.
fn conflicts(root: &Path, files: &[PathBuf], p: &mut Problems) {
    for f in files {
        let Ok(text) = std::fs::read_to_string(f) else { continue };
        if let Some(n) =
            text.lines().position(|l| l.starts_with("<<<<<<< ") || l == "<<<<<<<" || l.starts_with(">>>>>>> "))
        {
            let rel = specfold_core::paths::rel_of(root, f).unwrap_or_else(|| specfold_core::paths::slash(f));
            p.add(
                "conflict",
                &rel,
                format!("merge conflict marker at line {}", n + 1),
                "resolve the merge, then spec check",
            );
        }
    }
}

/// Local documents git would commit.
fn local_ignored(root: &Path, p: &mut Problems) {
    let local = root.join(".spec.local");
    if root.join(".git").exists() && local.exists() {
        let ignored = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["check-ignore", "-q", ".spec.local/x.md"])
            .status()
            .map(|s| s.success())
            .unwrap_or(true);
        if !ignored {
            p.add(
                "local",
                ".spec.local",
                "local documents are not git-ignored".into(),
                "add .spec.local/ to .gitignore",
            );
        }
    }
}

/// Ids defined twice and references left behind by a half-finished move.
fn corpus_problems(g: &Global, p: &mut Problems) {
    // Corpus-level checks need the documents loaded, local ones included.
    let full = Global { no_local: false, ..g.clone() };
    let Ok(s) = session::open(&full) else { return };
    let c = &s.corpus;
    // The same id from two files: usually a local draft copied into .spec, or the reverse.
    let mut by_id: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for d in &c.docs {
        by_id.entry(d.id.to_string()).or_default().push(&d.path);
    }
    for (id, paths) in by_id.iter().filter(|(_, p)| p.len() > 1) {
        // C139: the id names both files, so the fix names one by its path.
        let fix = match paths.iter().find(|p| p.contains(".spec.local/")) {
            Some(local) => format!("keep one: delete the local copy, or spec mv {local} --slug <new-slug> --write"),
            None => format!("keep one, or spec mv {} --slug <new-slug> --write", paths[paths.len() - 1]),
        };
        p.add("duplicate", paths[0], format!("{id} is also defined in {}", paths[1..].join(", ")), &fix);
    }
    // A move that stopped halfway leaves references to the old id while a document of the
    // same type and key lives under another owner.
    for d in &c.docs {
        for l in &d.links {
            if !matches!(s.graph.resolve(&l.item), Resolution::Missing) {
                continue;
            }
            let Some((ty, rest)) = l.item.split_once(':') else { continue };
            let key = rest.rsplit('/').next().unwrap_or(rest);
            if let Some(to) = c.docs.iter().find(|x| x.id.ty == ty && x.id.key == key) {
                let to = to.id.to_string();
                p.add(
                    "moved",
                    &format!("{}:{}", l.path, l.line),
                    format!("refers to {}, which looks moved to {to}", l.item),
                    &format!("replace {} with {to}, then spec check", l.item),
                );
            }
        }
    }
}

/// Remotes are listed, not judged: an absent directory is normal in a CI job without that
/// checkout, so it is shown here and stays silent in spec check.
fn remotes(cfg: &specfold_core::config::Config, mut o: Output) -> Output {
    let mut remotes = vec![];
    for (name, r) in &cfg.raw.remotes {
        let dir = specfold_core::workspace::remotes::dir(cfg, name).unwrap_or_default();
        let docs = specfold_core::workspace::remotes::load(cfg, name).map(|l| l.corpus.docs.len());
        let state = match docs {
            Some(n) => format!("{n} documents"),
            None => "absent: references to it are not checked".into(),
        };
        o = o.foot(format!("remote {name:<10} {}  {state}", dir.display()));
        remotes.push(json!({"name": name, "path": r.path, "dir": dir, "present": docs.is_some(), "documents": docs}));
    }
    o.field("remotes", json!(remotes))
}

/// Markdown files under any spec directory (.spec or .spec.local), skipping build and VCS dirs.
fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if p.is_dir() {
            if !SKIP.contains(&name.as_str()) {
                walk(&p, out);
            }
        } else if name.ends_with(".md")
            && p.components().any(|c| matches!(c.as_os_str().to_str(), Some(".spec" | ".spec.local")))
        {
            out.push(p);
        }
    }
}

/// C235: where to edit a spec.toml that does not load: the line the parse error names.
fn config_fix(msg: &str) -> String {
    let line = msg.split("at line ").nth(1).and_then(|r| r.split(|c: char| !c.is_ascii_digit()).next());
    match line.filter(|n| !n.is_empty()) {
        Some(n) => format!("fix spec.toml at line {n} (spec explain config)"),
        None => "fix spec.toml (spec explain config)".into(),
    }
}

/// Each config warning gets the fix for its own kind of mistake.
fn warning_fix(w: &str) -> &'static str {
    if w.starts_with("[verify] shell") {
        "set [verify] shell to an installed shell such as sh or bash, or remove the key"
    } else if w.starts_with("[audit] coverage_types") {
        "correct the type name in [audit] coverage_types (the warning lists the types)"
    } else if w.contains("require_when") {
        "correct the status in require_when, or add it to the type's statuses"
    } else {
        "rename the table, or give the new type an `about`"
    }
}
