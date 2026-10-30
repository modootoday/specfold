//! The deterministic part of an audit: the same corpus and history give the same items.

mod coverage;
mod debt;
mod drift;
mod exceptions;
mod finished;
mod gap;
mod open_items;
mod orphan;
mod stale;
mod stalled;

use super::types::{Diagnosis, Kind};
use crate::corpus::Corpus;
use crate::graph::Graph;

#[derive(Debug, Clone, Default)]
pub struct Filter {
    /// Only documents of these owners; empty means all.
    pub owners: Vec<String>,
    /// Only these kinds; empty means all.
    pub kinds: Vec<Kind>,
}

pub(crate) fn days_between(from: &str, to: &str) -> Option<i64> {
    let f = time::macros::format_description!("[year]-[month]-[day]");
    let a = time::Date::parse(&crate::model::stamp::utc_day(from)?, f).ok()?;
    let b = time::Date::parse(&crate::model::stamp::utc_day(to)?, f).ok()?;
    Some((b - a).whole_days())
}

/// The command that archives document `i`. A plan with open tasks is refused by `spec set`
/// (C119), so its next shows the tasks first and `--force` as the way to archive anyway.
pub(super) fn archive(c: &Corpus, i: usize) -> String {
    use crate::markdown::{frontmatter, tasks};
    let d = &c.docs[i];
    let open: usize = if d.id.ty == "plan" {
        std::iter::once(&d.path)
            .chain(&d.chapters)
            .filter_map(|f| std::fs::read_to_string(c.cfg.root.join(f)).ok())
            .map(|t| tasks::list(frontmatter::split(&t).body).into_iter().filter(|t| !t.done).count())
            .sum()
    } else {
        0
    };
    if open == 0 {
        format!("spec set {} status=archived", crate::shell::word(&d.id))
    } else {
        format!(
            "spec show {} --tasks   # {open} open task(s): tick or drop them, or archive anyway: spec set {} status=archived --write --force",
            crate::shell::word(&d.id),
            crate::shell::word(&d.id)
        )
    }
}

pub fn run(c: &Corpus, g: &Graph, filter: &Filter) -> Diagnosis {
    let docs: Vec<usize> = (0..c.docs.len())
        .filter(|&i| filter.owners.is_empty() || filter.owners.contains(&c.docs[i].id.owner))
        .collect();
    let owners: Vec<String> = c
        .owners
        .list
        .iter()
        .map(|o| o.id.clone())
        .filter(|o| filter.owners.is_empty() || filter.owners.contains(o))
        .collect();
    let want = |k: Kind| filter.kinds.is_empty() || filter.kinds.contains(&k);
    let mut items = vec![];
    if want(Kind::Stale) {
        stale::run(c, &docs, &mut items);
    }
    if want(Kind::Orphan) {
        orphan::run(c, g, &docs, &mut items);
    }
    if want(Kind::Gap) {
        gap::run(c, &owners, &mut items);
    }
    if want(Kind::Stalled) {
        stalled::run(c, &docs, &mut items);
    }
    if want(Kind::Finished) {
        finished::run(c, &docs, &mut items);
    }
    if want(Kind::OpenItem) {
        open_items::run(c, &docs, &mut items);
    }
    if want(Kind::Coverage) {
        coverage::run(c, g, &docs, &mut items);
    }
    if want(Kind::Exception) {
        exceptions::run(c, &docs, &mut items);
    }
    if want(Kind::Drift) {
        drift::run(c, &mut items);
    }
    if want(Kind::Debt) {
        debt::run(c, g, &docs, &mut items);
    }
    items.sort_by(|a, b| (a.kind, &a.path, a.line).cmp(&(b.kind, &b.path, b.line)));
    Diagnosis { items, docs: docs.len(), owners: owners.len() }
}
