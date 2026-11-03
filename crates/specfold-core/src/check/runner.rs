//! Runs the selected rules and orders their findings.

use super::context::{Ctx, Options};
use super::finding::Finding;
use super::rules::catalog;
use super::scope::{Range, Scope};
use crate::corpus::Corpus;
use crate::corpus::graph::Graph;
use crate::plugins;
use anyhow::Result;
use rayon::prelude::*;

#[derive(Debug, Clone, Default)]
pub struct Selection {
    /// Only these rule ids; empty means all.
    pub only: Vec<String>,
    /// Skip these rule ids.
    pub skip: Vec<String>,
}

pub struct Report {
    pub findings: Vec<Finding>,
    pub rules_run: Vec<&'static str>,
    pub docs_checked: usize,
    /// D2: the paths a ranged or staged run looked at (changed files and the files of the
    /// documents it checked); `None` for a full run, which looks at everything.
    pub checked_paths: Option<std::collections::BTreeSet<String>>,
}

impl Report {
    /// D2: whether this run judged the baseline key `RULE<TAB>path`. A ranged run reports nothing
    /// outside its range and no run reports a rule it skipped, so only such entries can be said
    /// to hold fewer errors now. Plugin rules always run with the rest.
    pub fn judged(&self, key: &str) -> bool {
        let (rule, path) = key.split_once('\t').unwrap_or((key, ""));
        (rule.starts_with("x.") || self.rules_run.contains(&rule))
            && self.checked_paths.as_ref().is_none_or(|p| p.contains(path))
    }
}

pub fn run(c: &Corpus, g: &Graph, range: Range, opts: &Options, sel: &Selection) -> Result<Report> {
    let t = std::time::Instant::now();
    let scope = Scope::build(c, g, range)?;
    let t = crate::trace::phase("scope", t);
    let ctx = Ctx { corpus: c, graph: g, scope: &scope, opts };
    let staged = matches!(scope.range, Range::Staged);
    let rules: Vec<_> = catalog()
        .into_iter()
        .filter(|r| {
            let id = r.info().id;
            (sel.only.is_empty() || sel.only.iter().any(|o| o == id))
                && !sel.skip.iter().any(|s| s == id)
                && (!staged || r.info().on_commit)
        })
        .collect();
    let mut findings: Vec<Finding> = rules
        .par_iter()
        .flat_map(|r| {
            let mut out = vec![];
            let t = std::time::Instant::now();
            r.run(&ctx, &mut out);
            crate::trace::rule(r.info().id, t);
            out
        })
        .collect();
    let wants_plugins = sel.only.is_empty() || sel.only.iter().any(|o| o.starts_with("x."));
    if wants_plugins && !c.cfg.raw.plugins.is_empty() && !sel.skip.iter().any(|s| s == "x") {
        let loaded = plugins::load(&c.cfg);
        plugins::rules::run(&ctx, &loaded, &mut findings);
    }
    let t = crate::trace::phase("rules", t);
    let mut findings = super::severity::apply(c, findings);
    crate::trace::phase("severity", t);
    // A secret comes first among findings of its level: it is what to fix before anything else,
    // and an attachment that changed because of it (E1) should not be re-attached first.
    let secret = |f: &Finding| f.rule == "U2" || (f.rule == "U1" && f.message.starts_with("[secrets]"));
    findings.sort_by(|a, b| {
        (a.severity, !secret(a), &a.path, a.line, &a.rule).cmp(&(b.severity, !secret(b), &b.path, b.line, &b.rule))
    });
    findings.dedup_by(|a, b| a.rule == b.rule && a.path == b.path && a.line == b.line && a.message == b.message);
    let docs_checked = if scope.is_all() { c.docs.len() } else { scope.with_neighbours.len() };
    let checked_paths = (!scope.is_all()).then(|| {
        let docs =
            scope.with_neighbours.iter().flat_map(|&i| std::iter::once(&c.docs[i].path).chain(&c.docs[i].chapters));
        scope.paths.iter().chain(&scope.gone).chain(docs).cloned().collect()
    });
    Ok(Report { findings, rules_run: rules.iter().map(|r| r.info().id).collect(), docs_checked, checked_paths })
}
