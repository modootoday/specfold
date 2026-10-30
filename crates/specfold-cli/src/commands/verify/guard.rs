//! `spec guard`.

use crate::cli::args::{global::Global, verify::GuardArgs};
use crate::output::{Code, Output, findings};
use crate::session;
use anyhow::Result;
use serde_json::json;
use specfold_core::guards::{self, GuardRange};

pub fn run(g: &Global, a: &GuardArgs) -> Result<Output> {
    let cfg = session::config(g)?;
    if a.write_baseline {
        // A baseline hides today's hits from every later run: --yes confirms it (CLI-3).
        let mut o = Output::new("guard").field("written", a.yes);
        let mut total = 0;
        for (name, rel, hits) in guards::write_baselines(&cfg, &a.names, a.yes)? {
            total += hits;
            let verb = if a.yes { "" } else { "would record " };
            o = o.item(
                json!({"guard": name, "baseline": rel, "hits": hits}),
                format!("{name}: {verb}{hits} hit(s) in {rel}"),
            );
        }
        if !a.yes {
            let names = if a.names.is_empty() { String::new() } else { format!(" {}", a.names.join(" ")) };
            return Ok(o
                .field("baselined", total)
                .head(format!(
                    "would record {total} hit(s) as baselines, hiding them from later runs: nothing was written"
                ))
                .next(format!("spec guard{names} --write-baseline --yes")));
        }
        return Ok(o.next("git add <baseline files>"));
    }
    let range = if a.staged {
        Some(GuardRange::Staged)
    } else if a.tree {
        Some(GuardRange::All)
    } else {
        a.range.as_deref().map(GuardRange::parse)
    };
    // `secrets` is the built-in check `spec export` and U2 run, unless a guard takes that name.
    let configured = |n: &str| cfg.raw.guards.iter().any(|g| g.name == n);
    let asked_builtin = a.names.iter().any(|n| n == BUILTIN && !configured(n));
    let names: Vec<String> = a.names.iter().filter(|n| *n != BUILTIN || configured(n)).cloned().collect();
    let builtin = asked_builtin || (a.names.is_empty() && !cfg.raw.guards.iter().any(|g| g.secrets));
    let mut unread = None;
    let mut list = if names.is_empty() && asked_builtin { vec![] } else { guards::run(&cfg, &names, range.as_ref())? };
    if builtin {
        let (hits, note) = builtin_hits(g, range.as_ref())?;
        unread = note;
        for f in hits {
            if !list.iter().any(|x| x.path == f.path && x.line == f.line) {
                list.push(f);
            }
        }
    }
    let (errors, warnings) = findings::counts(&list);
    let o = findings::push_all(Output::new("guard").field("errors", errors).field("warnings", warnings), &list);
    let mut shown: Vec<String> =
        if a.names.is_empty() { cfg.raw.guards.iter().map(|g| g.name.clone()).collect() } else { names.clone() };
    if builtin {
        shown.push("secrets (built-in)".into());
    }
    let names = shown.join(", ");
    let o = o.foot(format!("guards {names}: {errors} error(s), {warnings} warning(s)"));
    let o = match unread {
        Some(note) => o.field("not_scanned", note.clone()).foot(note),
        None => o,
    };
    Ok(if errors > 0 {
        o.code(Code::Findings).next(if list.iter().all(|f| f.rule == "U2") {
            "spec explain U2"
        } else {
            "spec explain U1"
        })
    } else {
        o
    })
}

const BUILTIN: &str = "secrets";

/// The built-in secret check over document files in the range (the whole tree without one).
/// Also returns one line naming the attachments it skipped (over the size limit or binary).
fn builtin_hits(
    g: &Global,
    range: Option<&GuardRange>,
) -> Result<(Vec<specfold_core::check::Finding>, Option<String>)> {
    let s = session::open(g)?;
    let root = &s.corpus.cfg.root;
    // Each document's files: the main file, its chapters and its repository text attachments.
    let max = s.corpus.cfg.raw.files.scan_max_bytes;
    let unread = std::cell::RefCell::new(Vec::<String>::new());
    let own = |d: &specfold_core::model::Doc| -> Vec<String> {
        let (read, skipped) = guards::secrets::attachment_scan(root, d, max);
        unread.borrow_mut().extend(skipped);
        std::iter::once(d.path.clone()).chain(d.chapters.iter().cloned()).chain(read).collect()
    };
    let docs: Vec<String> = s.corpus.docs.iter().flat_map(own).collect();
    let local: Vec<String> =
        s.corpus.docs.iter().filter(|d| d.tier == specfold_core::model::Tier::Local).flat_map(own).collect();
    let mut in_range: Option<Vec<String>> = None;
    let blobs: Vec<(String, Vec<u8>)> = match range {
        None | Some(GuardRange::All) => {
            docs.into_iter().filter_map(|p| Some((p.clone(), std::fs::read(root.join(&p)).ok()?))).collect()
        }
        Some(r) => {
            let res = guards::range::resolve(root, r, &[])?;
            in_range = Some(res.files.clone());
            res.files
                .into_iter()
                .filter(|f| docs.contains(f))
                .filter_map(|p| Some((p.clone(), guards::range::read(root, &res.source, &p)?)))
                .collect()
        }
    };
    let mut unread = unread.into_inner();
    unread.sort();
    unread.dedup();
    // In a range, only skipped files the range touches are worth a line.
    if let Some(files) = &in_range {
        unread.retain(|p| files.contains(p));
    }
    let note = guards::secrets::skipped_note(root, &unread, max);
    let hits = guards::builtin_secrets(&blobs)
        .into_iter()
        .map(|(_, mut f)| {
            f.rule = "U2".into();
            // As in U2: a local document never leaves the machine, so its hit is a warning.
            if local.contains(&f.path) {
                f.severity = specfold_core::check::Severity::Warn;
            }
            f
        })
        .collect();
    Ok((hits, note))
}
