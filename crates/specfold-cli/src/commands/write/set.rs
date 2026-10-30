//! `spec set`.

use super::preview;
use crate::cli::args::{global::Global, write::SetArgs};
use crate::output::Output;
use crate::session;
use anyhow::{Result, anyhow, bail};
use specfold_core::authoring::set::{self, Assign, Op};
use specfold_core::translation;

pub fn run(g: &Global, a: &SetArgs) -> Result<Output> {
    // CLI-1: from 0.4 `set` previews unless --write; 0.3 still writes, and says so on stderr.
    if !a.dry_run && !a.write {
        eprintln!(
            "warning: spec set writes without --write in 0.3; from 0.4 it previews unless --write. Add --write to keep writing (--dry-run previews)."
        );
    }
    let s = session::open(g)?;
    let idx = session::resolve(&s, &a.doc)?;
    let mut assigns = a.assignments.iter().map(|x| Assign::parse(x)).collect::<Result<Vec<_>>>()?;
    let mut shown = a.assignments.clone();
    crate::commands::outbound::refuse_redacted(
        &s.corpus.cfg.root,
        &s.corpus.docs[idx].path,
        &a.assignments.join("\n"),
    )?;
    if a.source_rev {
        let d = &s.corpus.docs[idx];
        let remote =
            translation::source_ref(&s.corpus, idx).and_then(|r| specfold_core::remotes::rev(&s.corpus.cfg, &r));
        let src = translation::source_of(&s.corpus, &s.graph, idx);
        if let (None, Some((_, rev))) = (src, remote) {
            shown.push(format!("{}={rev}", translation::KEY));
            assigns.push(Assign { key: translation::KEY.into(), op: Op::Set, value: rev });
        } else if let Some(src) = src {
            let rev = translation::rev_of(&s.corpus, src)
                .ok_or_else(|| anyhow!("cannot read {}", s.corpus.docs[src].path))?;
            shown.push(format!("{}={rev}", translation::KEY));
            assigns.push(Assign { key: translation::KEY.into(), op: Op::Set, value: rev });
        } else {
            bail!(
                "{} has no rel.translation_of that names a document; add it first: spec set {} rel.translation_of+=<source id>",
                d.id,
                d.id
            );
        }
    }
    for target in &a.pin {
        let Some((rid, rev)) = specfold_core::remotes::rev(&s.corpus.cfg, target) else {
            bail!("{}", pin_problem(&s, target));
        };
        let key = format!("pins.{rid}");
        shown.push(format!("{key}={rev}"));
        assigns.push(Assign { key, op: Op::Set, value: rev });
    }
    for target in &a.unpin {
        let d = &s.corpus.docs[idx];
        let pins: Vec<String> = d
            .fm()
            .and_then(|m| m.get("pins"))
            .and_then(|p| p.as_object())
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        let key = specfold_core::remotes::rev(&s.corpus.cfg, target).map(|(rid, _)| rid);
        let Some(key) = key.filter(|k| pins.contains(k)).or_else(|| pins.iter().find(|p| *p == target).cloned()) else {
            bail!(
                "{} has no pin {target} (pinned: {}); next: spec show {}",
                d.id,
                if pins.is_empty() { "none".to_string() } else { pins.join(", ") },
                d.id
            );
        };
        shown.push(format!("pins-={key}"));
        assigns.push(Assign { key: "pins".into(), op: Op::Remove, value: key });
    }
    let me = s.corpus.docs[idx].id.to_string();
    for x in &assigns {
        if x.key == "rel.exception_of"
            && !matches!(x.op, Op::Remove)
            && x.value.split(',').any(|v| session::resolve(&s, v.trim()).is_ok_and(|j| j == idx))
        {
            bail!("{me} cannot be an exception to itself; name the rule or decision it departs from");
        }
    }
    for x in assigns.iter().filter(|x| matches!(x.op, Op::Set)) {
        let problem = match x.key.as_str() {
            "lang" => translation::lang_problem(&x.value),
            "review_by" if !specfold_core::check::rules::is_date(&x.value) => {
                Some(format!("review_by '{}' is not a date (YYYY-MM-DD), for example review_by=2027-03-31", x.value))
            }
            translation::KEY => translation::rev_problem(&x.value).map(|p| format!("{p}; record it with --source-rev")),
            _ => None,
        };
        if let Some(p) = problem {
            bail!("{p}");
        }
    }
    let plan = set::plan(&s.corpus, idx, &assigns)?;
    // A plan is archived when its work is done: open tasks refuse the move unless --force.
    let open = open_tasks(&s.corpus, idx, &assigns);
    let id = s.corpus.docs[idx].id.to_string();
    let ty = s.corpus.docs[idx].id.ty.clone();
    let unknown: Vec<String> =
        assigns.iter().filter_map(|x| specfold_core::check::rules::unknown_key(&s.corpus, &ty, &x.key)).collect();
    let o = Output::new("set").unpaged().field("id", id.clone()).head(format!("{id}: {}", shown.join(" ")));
    if !open.is_empty() && !a.force {
        let shown_open: Vec<String> = open.iter().take(5).map(|t| format!("open  {t}")).collect();
        let more = open.len().saturating_sub(5);
        let o = o
            .field("open_tasks", open.clone())
            .foot(format!(
                "{} {} still open; a plan is archived when its tasks are done{}",
                open.len(),
                if open.len() == 1 { "task is" } else { "tasks are" },
                if more > 0 { format!(" ({more} more not shown)") } else { String::new() }
            ))
            .next(format!(
                "spec show {id} --tasks   # tick or drop them; or archive anyway: spec set {id} {} --write --force",
                a.assignments.join(" ")
            ));
        let o = shown_open.into_iter().fold(o, |o, l| o.foot(l));
        return Ok(if a.dry_run { o } else { o.code(crate::output::Code::Refused) });
    }
    let o = preview::finish(o, &plan, &s.corpus.cfg.root, !a.dry_run, "rerun without --dry-run")?;
    let o = unknown.iter().fold(o.field("warnings", unknown.clone()), |o, w| o.foot(format!("warn  S1  {w}")));
    Ok(if a.dry_run {
        o
    } else {
        {
            let mut rules = vec!["S1", "T1", "S4", "L3"];
            if !a.pin.is_empty() || !a.unpin.is_empty() || assigns.iter().any(|x| x.key.starts_with("pins")) {
                rules.push("L7");
            }
            // C111: a new status can require fields, and a required field's value is checked.
            let required: Vec<&String> =
                s.corpus.cfg.registry.get(&ty).map(|t| t.require_when.values().flatten().collect()).unwrap_or_default();
            if assigns.iter().any(|x| x.key == "status" || required.contains(&&x.key)) {
                rules.extend(["S6", "S8"]);
            }
            if assigns.iter().any(|x| x.key.starts_with("aliases")) {
                rules.push("I2");
            }
            if assigns.iter().any(|x| x.key == "review_by" || x.key == "rel.exception_of") {
                rules.push("T5");
            }
            let rules: Vec<String> = rules.iter().map(|r| format!("--rule {r}")).collect();
            o.next(format!("spec check {}  # then: spec show {id}", rules.join(" ")))
        }
    })
}

/// Why `--pin <target>` names nothing to pin, with the next step for that case.
fn pin_problem(s: &session::Session, target: &str) -> String {
    use specfold_core::remotes::{self, Remote};
    let cfg = &s.corpus.cfg;
    let known: Vec<&str> = cfg.raw.remotes.keys().map(String::as_str).collect();
    let known = if known.is_empty() { "none configured".to_string() } else { format!("known: {}", known.join(", ")) };
    match remotes::resolve(cfg, target) {
        Remote::Local => {
            if session::resolve(s, target).is_ok() {
                format!(
                    "{target} is a document in this repository, not a remote one; pins are for <remote>:<id> ({known}); next: spec set <id> rel.<kind>+={target}"
                )
            } else if let Some((name, _)) = target.split_once(':').filter(|(n, _)| cfg.registry.get(n).is_none()) {
                format!(
                    "'{name}' is not a configured remote ({known}); next: add [remotes.{name}] path = \"<dir>\" to spec.toml"
                )
            } else {
                format!("{target} is not a remote reference <remote>:<id> ({known}); next: spec doctor")
            }
        }
        Remote::Absent { name, path } => {
            format!(
                "the remote {name} is not on disk ({}); check it out there first; next: spec doctor",
                path.display()
            )
        }
        Remote::Missing { name, id } | Remote::MissingAnchor { name, id, .. } => {
            format!("the remote {name} has no document {id}; next: spec find <words> in that repository")
        }
        Remote::Ambiguous { name, ids } => format!("{target} is ambiguous in the remote {name}: {}", ids.join(", ")),
        Remote::Found { .. } => format!("{target} has no readable body to pin"),
    }
}

/// The open task lines of a plan that `assigns` archives; empty for anything else.
fn open_tasks(c: &specfold_core::corpus::Corpus, idx: usize, assigns: &[Assign]) -> Vec<String> {
    use specfold_core::markdown::{frontmatter, tasks};
    let d = &c.docs[idx];
    let archives = assigns.iter().any(|x| x.key == "status" && matches!(x.op, Op::Set) && x.value == "archived");
    if d.id.ty != "plan" || !archives {
        return vec![];
    }
    let mut open = vec![];
    for file in std::iter::once(&d.path).chain(&d.chapters) {
        let Ok(text) = std::fs::read_to_string(c.cfg.root.join(file)) else { continue };
        let split = frontmatter::split(&text);
        for t in tasks::list(split.body).into_iter().filter(|t| !t.done) {
            open.push(format!("{file}:{}: {}", split.body_line + t.line - 1, t.text));
        }
    }
    open
}
