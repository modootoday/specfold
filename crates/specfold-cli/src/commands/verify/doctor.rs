//! `spec doctor`: problems that stop the other commands from working well, found without writing
//! anything. Rules judge documents; doctor judges the setup around them.

use crate::cli::args::global::Global;
use crate::output::{Code, Output};
use crate::session;
use anyhow::Result;
use serde_json::json;
use std::path::{Path, PathBuf};

const SKIP: [&str; 4] = [".git", "target", "node_modules", ".cargo-target"];

pub fn run(g: &Global) -> Result<Output> {
    let mut o = Output::new("doctor");
    let mut found = 0;
    let mut add = |o: Output, check: &str, path: &str, msg: String, fix: &str| {
        found += 1;
        o.item(
            json!({"check": check, "path": path, "message": msg, "fix": fix}),
            format!("{check:<9} {path}  {msg}\n          fix: {fix}"),
        )
    };
    let cfg = match session::config(g) {
        Ok(c) => c,
        Err(e) => {
            let o = add(o, "config", "spec.toml", format!("{e:#}"), "spec init, or fix spec.toml");
            return Ok(o.code(Code::Findings).next("spec explain config"));
        }
    };
    let root = cfg.root.clone();
    let mut files = vec![];
    walk(&root, &mut files);
    for f in &files {
        let Ok(text) = std::fs::read_to_string(f) else { continue };
        if let Some(n) =
            text.lines().position(|l| l.starts_with("<<<<<<< ") || l == "<<<<<<<" || l.starts_with(">>>>>>> "))
        {
            let rel = f.strip_prefix(&root).unwrap_or(f).to_string_lossy().replace('\\', "/");
            o = add(
                o,
                "conflict",
                &rel,
                format!("merge conflict marker at line {}", n + 1),
                "resolve the merge, then spec check",
            );
        }
    }
    let local = root.join(".spec.local");
    if root.join(".git").exists() && local.exists() {
        let ignored = std::process::Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(["check-ignore", "-q", ".spec.local/x.md"])
            .status()
            .map(|s| s.success())
            .unwrap_or(true);
        if !ignored {
            o = add(
                o,
                "local",
                ".spec.local",
                "local documents are not git-ignored".into(),
                "add .spec.local/ to .gitignore",
            );
        }
    }
    o = o.head(format!("{} spec files read", files.len()));
    Ok(if found == 0 { o.foot("no setup problems found").next("spec check") } else { o.code(Code::Findings) })
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
