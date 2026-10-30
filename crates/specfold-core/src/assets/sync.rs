//! `spec sync assets`: copies the source of each `[[assets]]` root that declares `copy_of` over
//! that root. The only place specfold writes an asset. Writes stay inside the declared copy root,
//! never touch the source, never follow a symbolic link, and delete only with `prune`.

use crate::authoring::{Change, Plan};
use crate::config::Config;
use anyhow::{Result, bail};
use std::collections::BTreeSet;
use std::path::{Component, Path};

/// One root that will be synced: the copy and its source, both repository-relative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pair {
    pub source: String,
    pub copy: String,
}

/// The pairs declared in spec.toml, validated; an unsafe declaration fails the whole sync.
pub fn pairs(cfg: &Config) -> Result<Vec<Pair>> {
    let mut out = vec![];
    for r in &cfg.raw.assets {
        let Some(src) = &r.copy_of else { continue };
        let copy = clean(&r.path, "path")?;
        let source = clean(src, "copy_of")?;
        if inside(&copy, &source) || inside(&source, &copy) {
            bail!("[[assets]] path = \"{copy}\" and copy_of = \"{source}\" overlap; a copy must sit beside its source");
        }
        no_links(&cfg.root, &source)?;
        no_links(&cfg.root, &copy)?;
        if !cfg.root.join(&source).is_dir() {
            bail!("[[assets]] copy_of = \"{source}\" is not a directory");
        }
        out.push(Pair { source, copy });
    }
    for (i, a) in out.iter().enumerate() {
        if let Some(b) = out[..i].iter().find(|b| inside(&a.copy, &b.copy) || inside(&b.copy, &a.copy)) {
            bail!("[[assets]] copy roots \"{}\" and \"{}\" overlap", b.copy, a.copy);
        }
        if let Some(b) = out.iter().find(|b| inside(&b.source, &a.copy) || inside(&a.copy, &b.source)) {
            bail!("[[assets]] copy root \"{}\" overlaps the source \"{}\"", a.copy, b.source);
        }
    }
    Ok(out)
}

/// The changes that make every copy root equal to its source, and the copies among them that
/// would overwrite a file with no sign sync wrote it (CLI-4): an existing copy is sync's own when
/// its content is a version its source has had (in git history or now).
pub fn plan(cfg: &Config, prune: bool) -> Result<(Vec<Pair>, Plan, Vec<String>)> {
    let pairs = pairs(cfg)?;
    let mut plan = Plan::default();
    let mut unowned = vec![];
    for p in &pairs {
        let src = files(&cfg.root, &p.source)?;
        for rel in &src {
            let (from, to) = (format!("{}/{rel}", p.source), format!("{}/{rel}", p.copy));
            no_links(&cfg.root, &to)?;
            let same = std::fs::read(cfg.root.join(&from)).ok() == std::fs::read(cfg.root.join(&to)).ok();
            if !same {
                if cfg.root.join(&to).is_file() && !owned(&cfg.root, &from, &to) {
                    unowned.push(to.clone());
                }
                plan.changes.push(Change::Copy { from, to });
            }
        }
        if prune && cfg.root.join(&p.copy).is_dir() {
            for rel in files(&cfg.root, &p.copy)?.difference(&src) {
                plan.changes.push(Change::Remove { path: format!("{}/{rel}", p.copy) });
            }
        }
    }
    Ok((pairs, plan, unowned))
}

/// Whether the copy at `to` holds a version of `from`: what sync wrote and nobody changed since.
fn owned(root: &Path, from: &str, to: &str) -> bool {
    use crate::workspace::vcs;
    let Some(id) = vcs::blob_id(root, to) else { return false };
    vcs::past_blobs(root, from).contains(&id)
}

/// A relative path inside the repository: no root, no `..`, no glob, not the repository itself.
fn clean(p: &str, key: &str) -> Result<String> {
    let t = p.trim().trim_end_matches('/').replace('\\', "/");
    let path = Path::new(&t);
    let ok = !t.is_empty()
        && !t.contains(['*', '?', '[', '{'])
        && path.components().all(|c| matches!(c, Component::Normal(_)) || c == Component::CurDir)
        && path.components().any(|c| matches!(c, Component::Normal(_)));
    if !ok {
        bail!("[[assets]] {key} = \"{p}\" must be a directory inside the repository (relative, no '..', no glob)");
    }
    Ok(t.trim_start_matches("./").to_string())
}

fn inside(a: &str, b: &str) -> bool {
    a == b || a.starts_with(&format!("{b}/"))
}

/// Refuses a path whose existing part passes through a symbolic link, which could lead outside.
fn no_links(root: &Path, rel: &str) -> Result<()> {
    let mut cur = root.to_path_buf();
    for part in rel.split('/') {
        cur.push(part);
        match std::fs::symlink_metadata(&cur) {
            Ok(m) if m.file_type().is_symlink() => {
                bail!("{rel}: passes through a symbolic link; sync does not follow links")
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    Ok(())
}

/// Every file below `dir`, relative to it; a symbolic link anywhere below is refused.
fn files(root: &Path, dir: &str) -> Result<BTreeSet<String>> {
    let mut out = BTreeSet::new();
    let mut stack = vec![String::new()];
    while let Some(sub) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(root.join(dir).join(&sub)) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let rel = if sub.is_empty() { name } else { format!("{sub}/{name}") };
            let ft = e.file_type()?;
            if ft.is_symlink() {
                bail!("{dir}/{rel}: is a symbolic link; sync does not follow links");
            } else if ft.is_dir() {
                stack.push(rel);
            } else {
                out.insert(rel);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_relative_paths_inside_the_repository() {
        assert_eq!(clean("./.claude/skills/", "path").unwrap(), ".claude/skills");
        for bad in ["", ".", "/etc", "../x", "a/../../b", ".claude/*"] {
            assert!(clean(bad, "path").is_err(), "{bad}");
        }
        assert!(inside(".agents/skills/x", ".agents/skills") && !inside(".agents/skillsx", ".agents/skills"));
    }
}
