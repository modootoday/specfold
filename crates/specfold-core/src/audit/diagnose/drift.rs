//! Agent assets that drifted from their source: a skill copy under a tool directory that
//! differs from (or is missing in) `.agents/`, and a CLAUDE.md that does not include AGENTS.md.

use super::super::types::{Item, Kind};
use crate::assets::{self, Asset};
use crate::config::types::AssetKind;
use crate::corpus::Corpus;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;

const CANONICAL: &str = ".agents/";

pub fn run(c: &Corpus, out: &mut Vec<Item>) {
    let root = &c.cfg.root;
    claude_includes_agents(root, out);
    let skills: Vec<Asset> = assets::load(&c.cfg).into_iter().filter(|a| a.kind == AssetKind::Skill).collect();
    let canonical: BTreeMap<&str, &Asset> =
        skills.iter().filter(|a| a.path.starts_with(CANONICAL)).map(|a| (a.stem.as_str(), a)).collect();
    if canonical.is_empty() {
        return;
    }
    let synced: Vec<&str> =
        c.cfg.raw.assets.iter().filter(|r| r.copy_of.is_some()).map(|r| r.path.trim_end_matches('/')).collect();
    for a in skills.iter().filter(|a| !a.path.starts_with(CANONICAL)) {
        let sync = synced.iter().any(|r| a.path.starts_with(&format!("{r}/")));
        let (detail, next) = match canonical.get(a.stem.as_str()) {
            None => (
                format!("skill '{}' has no source under .agents/skills", a.stem),
                format!("move {} to .agents/skills/{}/ and keep this as a copy", parent(&a.path), a.stem),
            ),
            Some(src) if digest(root, &a.path) != digest(root, &src.path) => (
                format!("differs from {}, the canonical copy", parent(&src.path)),
                if sync {
                    "spec sync assets --write".to_string()
                } else {
                    format!("diff -r {} {}", parent(&src.path), parent(&a.path))
                },
            ),
            Some(_) => continue,
        };
        out.push(Item {
            kind: Kind::Drift,
            subject: a.stem.clone(),
            path: a.path.clone(),
            line: 1,
            detail,
            next: Some(next),
        });
    }
}

fn claude_includes_agents(root: &Path, out: &mut Vec<Item>) {
    if !root.join("AGENTS.md").is_file() {
        return;
    }
    let Ok(text) = std::fs::read_to_string(root.join("CLAUDE.md")) else { return };
    if text.lines().any(|l| l.trim_start().starts_with("@AGENTS.md")) {
        return;
    }
    out.push(Item {
        kind: Kind::Drift,
        subject: "CLAUDE.md".into(),
        path: "CLAUDE.md".into(),
        line: 1,
        detail: "does not include AGENTS.md, so the two can say different things".into(),
        next: Some("add a line '@AGENTS.md' to CLAUDE.md".into()),
    });
}

fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(d, _)| d)
}

/// Hash of every file in the skill's directory, by relative path.
fn digest(root: &Path, skill_md: &str) -> Vec<u8> {
    let dir = root.join(parent(skill_md));
    let mut files: Vec<_> = walkdir(&dir);
    files.sort();
    let mut h = Sha256::new();
    for f in files {
        let rel = f.strip_prefix(&dir).unwrap_or(&f).to_string_lossy().replace('\\', "/");
        h.update(rel.as_bytes());
        h.update([0]);
        h.update(std::fs::read(&f).unwrap_or_default());
        h.update([0]);
    }
    h.finalize().to_vec()
}

fn walkdir(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = vec![];
    let Ok(rd) = std::fs::read_dir(dir) else { return out };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walkdir(&p));
        } else {
            out.push(p);
        }
    }
    out
}
