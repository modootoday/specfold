//! D1: `spec check --changed <base> --owners`, the push gate of a repository holding several
//! projects. Errors fail the run only in the owners the change touches; errors other owners
//! already hold are reported (owner, path and the submodule they sit in) without failing.

use crate::cli::args::verify::CheckArgs;
use crate::commands::session::Session;
use crate::output::Output;
use anyhow::Result;
use specfold_core::check::{self, Finding, Options, Range, Selection, Severity, baseline};
use specfold_core::workspace::vcs;
use std::collections::{BTreeMap, BTreeSet};

/// Errors of an owner the change does not touch: its path and how many.
pub type Others = BTreeMap<String, (String, usize)>;

fn key(f: &Finding) -> (String, String, usize, String) {
    (f.rule.clone(), f.path.clone(), f.line, f.message.clone())
}

/// With `--owners`, adds every finding of the touched owners to `scoped` and counts the errors
/// of the others; without it, returns `scoped` unchanged.
pub fn widen(
    s: &Session,
    a: &CheckArgs,
    scoped: Vec<Finding>,
    base: Option<&baseline::Baseline>,
    opts: &Options,
) -> Result<(Vec<Finding>, Others)> {
    let root = &s.corpus.cfg.spec_home;
    let changed = match (&a.owners, &a.changed, a.staged) {
        (false, _, _) => None,
        (true, Some(b), _) => vcs::changed_since(root, b),
        (true, None, true) => vcs::staged(root),
        (true, None, false) => None,
    };
    let Some(changed) = changed else { return Ok((scoped, Others::new())) };
    let owners = &s.corpus.owners;
    let touched: BTreeSet<&str> = changed.iter().map(|p| owners.containing(p).id.as_str()).collect();
    let sel = Selection { only: a.rules.clone(), skip: a.skip.clone() };
    let full = check::run(&s.corpus, &s.graph, Range::All, opts, &sel)?.findings;
    let full = match base {
        Some(b) => baseline::hold(&s.corpus, b, full).findings,
        None => full,
    };
    let mut seen: BTreeSet<_> = scoped.iter().map(key).collect();
    let (mut list, mut others) = (scoped, Others::new());
    for f in full {
        let o = owners.containing(&f.path);
        if touched.contains(o.id.as_str()) {
            if seen.insert(key(&f)) {
                list.push(f);
            }
        } else if f.severity == Severity::Error {
            others.entry(o.id.clone()).or_insert_with(|| (o.path.clone(), 0)).1 += 1;
        }
    }
    Ok((list, others))
}

/// The `other_owners` field and one foot line per owner, naming the submodule (gitlink) it is in.
pub fn annotate(o: Output, root: &std::path::Path, others: &Others) -> Output {
    if others.is_empty() {
        return o;
    }
    let links = specfold_core::workspace::projects::submodules(root);
    let link_of = |p: &str| links.iter().find(|l| specfold_core::paths::is_under(p, l)).cloned();
    let json: serde_json::Map<_, _> = others
        .iter()
        .map(|(id, (p, n))| (id.clone(), serde_json::json!({"path": p, "errors": n, "gitlink": link_of(p)})))
        .collect();
    others.iter().fold(o.field("other_owners", json), |o, (id, (p, n))| {
        let place = if p.is_empty() { ".".to_string() } else { p.clone() };
        let sub = link_of(p).map(|l| format!(", submodule {l}")).unwrap_or_default();
        o.foot(format!(
            "{id} ({place}{sub}): {n} error(s) in an owner this change does not touch, not failing; next: spec check {place}"
        ))
    })
}
