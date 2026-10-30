//! Runs the selected rules and orders their findings.

use super::context::{Ctx, Options};
use super::finding::Finding;
use super::rules::catalog;
use super::scope::{Range, Scope};
use crate::corpus::Corpus;
use crate::graph::Graph;
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
}

pub fn run(c: &Corpus, g: &Graph, range: Range, opts: &Options, sel: &Selection) -> Result<Report> {
    let scope = Scope::build(c, g, range)?;
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
            r.run(&ctx, &mut out);
            out
        })
        .collect();
    let wants_plugins = sel.only.is_empty() || sel.only.iter().any(|o| o.starts_with("x."));
    if wants_plugins && !c.cfg.raw.plugins.is_empty() && !sel.skip.iter().any(|s| s == "x") {
        let loaded = plugins::load(&c.cfg);
        plugins::rules::run(&ctx, &loaded, &mut findings);
    }
    let mut findings = super::severity::apply(c, findings);
    // A secret comes first among findings of its level: it is what to fix before anything else,
    // and an attachment that changed because of it (E1) should not be re-attached first.
    let secret = |f: &Finding| f.rule == "U2" || (f.rule == "U1" && f.message.starts_with("[secrets]"));
    findings.sort_by(|a, b| {
        (a.severity, !secret(a), &a.path, a.line, &a.rule).cmp(&(b.severity, !secret(b), &b.path, b.line, &b.rule))
    });
    findings.dedup_by(|a, b| a.rule == b.rule && a.path == b.path && a.line == b.line && a.message == b.message);
    let docs_checked = if scope.is_all() { c.docs.len() } else { scope.with_neighbours.len() };
    Ok(Report { findings, rules_run: rules.iter().map(|r| r.info().id).collect(), docs_checked })
}
