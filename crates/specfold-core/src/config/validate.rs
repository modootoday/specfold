//! What a loaded spec.toml must hold before it is used: a schema this version reads, guard ranges,
//! owner patterns, unshared baselines, and (C237) regexes and globs that compile. A pattern that
//! does not compile used to be dropped where it was used, so the setting silently did nothing.

use super::defaults;
use super::types::RawConfig;
use crate::workspace::owners::matcher;
use anyhow::{Result, bail};
use regex::Regex;
use std::path::Path;

/// Refuses a spec.toml at `path` (its text and parsed form) that cannot be used as written.
pub fn loaded(path: &Path, text: &str, raw: &RawConfig) -> Result<()> {
    if raw.spec.schema > defaults::schema() {
        return Err(newer_schema(path, raw.spec.schema));
    }
    if raw.spec.schema != defaults::schema() {
        bail!(
            "{}: schema {} is older than this version reads ({}); preview the upgrade: spec init --upgrade",
            path.display(),
            raw.spec.schema,
            defaults::schema()
        );
    }
    // C141: one guard with a mistyped range would stop every `spec guard` run later.
    for g in &raw.guards {
        if !matches!(g.range.as_str(), "staged" | "push" | "all") && !g.range.contains("..") {
            bail!(
                "{}: guard '{}' has range = \"{}\"; use \"staged\", \"push\", \"all\" or a git range such as \"origin/main..HEAD\" (spec explain config)",
                path.display(),
                g.name,
                g.range
            );
        }
    }
    for k in &raw.owners.kinds {
        if let Some(why) = crate::workspace::owners::patterns::kind_pattern_problem(&k.pattern) {
            bail!("{}: {why}; next: spec explain config", path.display());
        }
    }
    if let Some(n) = raw.owners.nested.iter().find(|n| n.contains(['*', '?', '[', '{']) || n.starts_with("..")) {
        bail!(
            "{}: [owners] nested has \"{n}\"; name each nested project by its directory inside the repository, e.g. packages/cli; next: spec explain config",
            path.display()
        );
    }
    if let Some(why) = super::type_dirs::problem(raw) {
        bail!("{}: {why}; next: spec explain config", path.display());
    }
    if let Some((why, value)) = pattern_problem(raw) {
        let at = text.lines().position(|l| l.contains(&value)).map(|n| format!(" at line {}", n + 1));
        bail!("{}{}: {why}; next: spec explain config", path.display(), at.unwrap_or_default());
    }
    // C184: guards sharing a baseline file overwrite each other's entries on --write-baseline.
    let mut seen: Vec<(&str, String)> = Vec::new();
    for g in &raw.guards {
        let Some(b) = g.baseline.as_deref() else { continue };
        let b = same_file_key(path.parent().unwrap_or(Path::new(".")), b);
        if let Some((other, _)) = seen.iter().find(|(_, x)| *x == b) {
            bail!(
                "{}: guards '{other}' and '{}' both write the baseline file \"{b}\"; give each guard its own baseline file; next: spec explain config",
                path.display(),
                g.name
            );
        }
        seen.push((&g.name, b));
    }
    Ok(())
}

/// The refusal for a spec.toml whose schema is newer than this binary reads; `spec check` and
/// `spec init --upgrade` (C170) both give it.
pub fn newer_schema(path: &Path, schema: u32) -> anyhow::Error {
    anyhow::anyhow!(
        "{}: schema {schema} was written by a newer specfold (this version reads {}); install the latest spec",
        path.display(),
        defaults::schema()
    )
}

/// C186: a baseline path as one string per file: relative to the repository root, with `.` and
/// `..` resolved lexically, so `./b.json`, `x/../b.json` and `<root>/b.json` compare equal.
fn same_file_key(root: &Path, p: &str) -> String {
    // C194/C195: resolve the deepest part of the path that exists (symbolic links included)
    // before any lexical clean-up, then apply the rest; `lnk/sub/..` is judged where the
    // write would land. Kept root-relative so it compares with the lexical keys below.
    if let Ok(croot) = root.canonicalize()
        && let Some(out) = crate::paths::real_with_lexical_tail(&root.join(p))
    {
        return crate::paths::rel_of(&croot, &out).unwrap_or_else(|| out.to_string_lossy().into_owned());
    }
    let p = Path::new(p);
    let rel = p.strip_prefix(root).unwrap_or(p);
    crate::paths::normalize_keeping_up(rel.to_str().unwrap_or_default())
}

/// The first pattern that does not compile: why, and the value as written (to find its line).
fn pattern_problem(raw: &RawConfig) -> Option<(String, String)> {
    let regexes: [(&str, &[String]); 3] = [
        ("[verify] serial", &raw.verify.serial),
        ("[forbid] patterns", &raw.forbid.patterns),
        ("[cites] history", &raw.cites.history),
    ];
    for (key, list) in regexes {
        for r in list {
            if let Err(e) = Regex::new(r) {
                let why = e.to_string().lines().last().unwrap_or("").trim().to_string();
                return Some((format!("{key} has \"{r}\", which is not a regular expression ({why})"), r.clone()));
            }
        }
    }
    let mut globs: Vec<(String, &[String])> = vec![
        ("[verify] full_run_on".into(), &raw.verify.full_run_on),
        ("[forbid] paths".into(), &raw.forbid.paths),
        ("[check] count_paths".into(), &raw.check.count_paths),
        ("[check] link_ignore".into(), &raw.check.link_ignore),
        ("[cites] paths".into(), &raw.cites.paths),
        ("[cites] exclude".into(), &raw.cites.exclude),
        ("[public] paths".into(), &raw.public.paths),
        ("[owners] retired".into(), &raw.owners.retired),
        ("[owners] pending".into(), &raw.owners.pending),
    ];
    for o in &raw.check.overrides {
        globs.push((format!("[[check.override]] rule {} paths", o.rule), &o.paths));
    }
    for g in &raw.guards {
        globs.push((format!("guard '{}' paths", g.name), &g.paths));
        globs.push((format!("guard '{}' exclude", g.name), &g.exclude));
        globs.push((format!("guard '{}' deny_files", g.name), &g.deny_files));
    }
    for a in &raw.assets {
        globs.push(("[[assets]] path".into(), std::slice::from_ref(&a.path)));
        globs.push(("[[assets]] exclude".into(), &a.exclude));
    }
    for (key, list) in globs {
        if let Some(p) = list.iter().find(|p| matcher(p).is_none()) {
            return Some((
                format!("{key} has \"{p}\", which is not a glob (check its brackets and braces)"),
                p.clone(),
            ));
        }
    }
    None
}
