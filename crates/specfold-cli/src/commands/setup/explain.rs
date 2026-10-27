//! `spec explain`: the rule's own explanation is the documentation of record.

use crate::cli::args::{global::Global, setup::ExplainArgs};
use crate::output::Output;
use crate::session;
use anyhow::{Result, bail};
use serde_json::json;
use specfold_core::check::rules::catalog;
use specfold_core::plugins;

const CONFIG_HELP: &[(&str, &str)] = &[
    ("[spec] schema, dir", "Format version (1) and the name of each owner's document directory (.spec)."),
    (
        "[owners] root, kinds, manifests, retired",
        "The root owner id; owner kinds as pattern + id template ({1}, {2} …) + manifest requirement; the manifest list; directories whose code is gone.",
    ),
    (
        "[public] paths, companion",
        "Published paths that may hold no spec document, and the private owner each one's documents go to.",
    ),
    ("[local] path", "The git-ignored directory for drafts and private notes."),
    (
        "[external.<name>] path, mode",
        "Directories outside the repository (an agent's memory) whose citations are checked with --external.",
    ),
    (
        "[files] inline_max_bytes, store",
        "Size limit for attachments in the repository; the store for larger ones (dir or command).",
    ),
    (
        "[types.<name>]",
        "Override or disable a built-in type, or add one: key, form, statuses, initial, sealed, fields, template, transitions (from → [to], rule T2), require_when (status → required keys, rule S6).",
    ),
    ("[forbid] paths, patterns", "Retired layouts and identifiers that must not come back (rule Z1)."),
    ("[aliases] file", "The frozen alias table written by spec import."),
    ("[[guards]]", "Named checks over files and commit messages (rule U1, spec guard)."),
    ("[[plugins]]", "External programs speaking specfold.plugin.v1: rules, importers, exporters, audits, events."),
    ("[audit]", "Thresholds for stale, stalled and draft documents, and open-item markers."),
    (
        "[verify] timeout_ms, jobs, serial, full_run_on",
        "How spec verify runs commands: the default deadline, parallel workers, regexes for heavy commands run one at a time, and paths whose change runs every check.",
    ),
];

pub fn run(g: &Global, a: &ExplainArgs) -> Result<Output> {
    let mut o = Output::new("explain");
    match a.topic.as_deref() {
        Some("config") => {
            for (k, v) in CONFIG_HELP {
                o = o.item(json!({"key": k, "explain": v}), format!("{k}\n  {v}"));
            }
            return Ok(o.next("spec init"));
        }
        Some(t) if t.starts_with("x.") => {
            let cfg = session::config(g)?;
            for p in plugins::load(&cfg) {
                let Ok(d) = p.description else { continue };
                for r in d.provides.rules {
                    if format!("x.{}.{}", p.def.name, r.id) == t {
                        return Ok(o.item(
                            json!({"id": t, "title": r.title, "explain": r.explain}),
                            format!("{t}  {}\n\n{}", r.title, r.explain),
                        ));
                    }
                }
            }
            bail!("no plugin rule '{t}'");
        }
        _ => {}
    }
    let rules = catalog();
    let picked: Vec<_> =
        rules.iter().filter(|r| a.topic.as_deref().is_none_or(|t| r.info().id.eq_ignore_ascii_case(t))).collect();
    if picked.is_empty() {
        bail!("no rule '{}' (spec explain lists them all)", a.topic.clone().unwrap_or_default());
    }
    for r in picked {
        let i = r.info();
        let text = if a.topic.is_some() {
            format!("{}  {}\n\n{}\n\nruns on commit: {}", i.id, i.title, i.explain, i.on_commit)
        } else {
            format!("{:<4} {}", i.id, i.title)
        };
        o = o.item(json!({"id": i.id, "title": i.title, "explain": i.explain, "on_commit": i.on_commit}), text);
    }
    Ok(o.next("spec check --rule <id>"))
}
