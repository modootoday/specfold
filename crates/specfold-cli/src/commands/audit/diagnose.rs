//! `spec audit` (default): the deterministic diagnosis.

use crate::cli::args::verify::AuditArgs;
use crate::output::Output;
use crate::session::Session;
use anyhow::{Result, bail};
use serde_json::to_value;
use specfold_core::audit::{self, Diagnosis, Filter, Kind};

pub fn kind(s: &str) -> Result<Kind> {
    Ok(match s {
        "stale" => Kind::Stale,
        "orphan" => Kind::Orphan,
        "gap" => Kind::Gap,
        "stalled" => Kind::Stalled,
        "finished" => Kind::Finished,
        "open-item" => Kind::OpenItem,
        "debt" => Kind::Debt,
        "drift" => Kind::Drift,
        "plugin" => Kind::Plugin,
        other => bail!(
            "unknown audit kind '{other}' (stale, orphan, gap, stalled, finished, open-item, debt, drift, plugin)"
        ),
    })
}

pub fn diagnose(s: &Session, a: &AuditArgs) -> Result<Diagnosis> {
    let filter = Filter { owners: a.owners.clone(), kinds: a.kinds.iter().map(|k| kind(k)).collect::<Result<_>>()? };
    let mut diag = audit::diagnose::run(&s.corpus, &s.graph, &filter);
    if filter.kinds.is_empty() || filter.kinds.contains(&Kind::Plugin) {
        audit::plugins::run(&s.corpus, &s.graph, &mut diag.items);
    }
    Ok(diag)
}

pub fn render(diag: &Diagnosis) -> Result<Output> {
    let mut o = Output::new("audit").field("mode", "diagnose").field("docs", diag.docs).field("owners", diag.owners);
    for i in &diag.items {
        let loc = if i.line > 0 { format!("{}:{}", i.path, i.line) } else { i.path.clone() };
        let kind = format!("{:?}", i.kind).to_lowercase();
        let mut text = format!("{kind:<10} {:<36} {}\n           {loc}", i.subject, i.detail);
        if let Some(n) = &i.next {
            text.push_str(&format!("\n           next: {n}"));
        }
        o = o.item(to_value(i)?, text);
    }
    let o = o.foot(format!("{} item(s) over {} documents and {} owners", diag.items.len(), diag.docs, diag.owners));
    Ok(o.next("spec audit --pack   # documents to review together"))
}

/// The diagnosis as the body of an audit record.
pub fn as_markdown(diag: &Diagnosis) -> String {
    let mut body = String::from("## Diagnosis\n\n");
    for i in &diag.items {
        body.push_str(&format!("- {:?} {} ({}:{}): {}\n", i.kind, i.subject, i.path, i.line, i.detail));
    }
    body
}
