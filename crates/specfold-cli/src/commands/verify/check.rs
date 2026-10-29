//! `spec check`.

use crate::cli::args::{global::Global, verify::CheckArgs};
use crate::output::{Code, Output, findings};
use crate::session;
use anyhow::{Result, bail};
use specfold_core::check::{self, Finding, Options, Range, Selection, Severity, baseline};
use specfold_core::workspace::vcs;
use std::time::Instant;

/// `--fail-on` must name rules that exist, so a gate cannot silently wait on a rule that never fires.
fn check_fail_on(names: &[String]) -> Result<()> {
    let known: Vec<&str> = check::rules::catalog().iter().map(|r| r.info().id).collect();
    for n in names {
        if n != "any" && !n.starts_with("x.") && !known.contains(&n.as_str()) {
            bail!("--fail-on {n}: no such rule (spec explain lists them)");
        }
    }
    Ok(())
}

fn failing(list: &[Finding], fail_on: &[String]) -> usize {
    let counts = |f: &&Finding| fail_on.is_empty() || fail_on.iter().any(|r| r == "any" || r == &f.rule);
    list.iter().filter(|f| f.severity == Severity::Error).filter(counts).count()
}

pub fn run(g: &Global, a: &CheckArgs) -> Result<Output> {
    let started = Instant::now();
    check_fail_on(&a.fail_on)?;
    let s = session::open(g)?;
    let range = match (&a.changed, a.staged) {
        (Some(base), _) => Range::Changed(base.clone()),
        (None, true) => Range::Staged,
        (None, false) => Range::All,
    };
    let opts = Options { run_commands: a.run_commands, external: a.external, online: a.online, guards: a.guards };
    let report =
        check::run(&s.corpus, &s.graph, range, &opts, &Selection { only: a.rules.clone(), skip: a.skip.clone() })?;
    if a.write_baseline {
        let b = baseline::measure(&s.corpus, &report.findings)?;
        let rel = baseline::write(&s.corpus, &b)?;
        let total: usize = b.counts.values().sum();
        return Ok(Output::new("check")
            .field("baseline", rel.clone())
            .field("measured_on", b.measured_on.clone())
            .field("baselined", total)
            .foot(format!("{total} error(s) in {} rule/path pair(s) recorded in {rel}", b.counts.len()))
            .next(format!("git add {rel}")));
    }
    let base = if a.no_baseline { None } else { baseline::load(&s.corpus)? };
    let (list, known, lowerable) = match &base {
        Some(b) => {
            let held = baseline::hold(&s.corpus, b, report.findings);
            (held.findings, held.known, held.lowerable.len())
        }
        None => (report.findings, 0, 0),
    };
    let (errors, warnings) = findings::counts(&list);
    let fails = failing(&list, &a.fail_on);
    let ms = started.elapsed().as_millis();
    let mut o = Output::new("check")
        .field("errors", errors)
        .field("warnings", warnings)
        .field("docs_checked", report.docs_checked)
        .field("docs_total", s.corpus.docs.len())
        .field("rules", report.rules_run.clone())
        .field("elapsed_ms", ms as u64);
    if base.is_some() {
        o = o.field("baselined", known).field("lowerable", lowerable);
    }
    let mut by_rule: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
    for f in &list {
        let e = by_rule.entry(f.rule.clone()).or_default();
        if f.severity == Severity::Error { e.0 += 1 } else { e.1 += 1 }
    }
    o = o.field(
        "by_rule",
        by_rule
            .iter()
            .map(|(r, (e, w))| (r.clone(), serde_json::json!({"errors": e, "warnings": w})))
            .collect::<serde_json::Map<_, _>>(),
    );
    let held = if base.is_some() && known > 0 {
        format!(" · {known} more error(s) held by the baseline (--no-baseline shows them)")
    } else {
        String::new()
    };
    let o = if a.summary {
        by_rule.iter().fold(o, |o, (r, (e, w))| {
            o.item(
                serde_json::json!({"rule": r, "errors": e, "warnings": w}),
                format!("{r}  {e} error(s), {w} warning(s)"),
            )
        })
    } else if a.errors_only {
        let errs: Vec<Finding> = list.iter().filter(|f| f.severity == Severity::Error).cloned().collect();
        findings::push_all(o, &errs)
    } else {
        findings::push_all(o, &list)
    };
    let o = o.foot(format!(
        "{errors} error(s), {warnings} warning(s) in {} of {} documents{held} · {:.2}s",
        report.docs_checked,
        s.corpus.docs.len(),
        ms as f64 / 1000.0
    ));
    let mut o = o;
    if a.new {
        let committed: std::collections::HashSet<String> =
            vcs::committed(&s.corpus.cfg.root).unwrap_or_default().into_iter().collect();
        let fresh: Vec<_> = s.corpus.docs.iter().filter(|d| !committed.contains(&d.path)).collect();
        o = o
            .field(
                "new_docs",
                fresh.iter().map(|d| serde_json::json!({"id": d.id.to_string(), "path": d.path})).collect::<Vec<_>>(),
            )
            .foot(format!("{} document(s) not in the last commit", fresh.len()));
        for d in fresh {
            o = o.foot(format!("  new  {}  {}", d.id, d.path));
        }
    }
    let has_fix = list.iter().any(|f| f.fix.is_some());
    Ok(match (fails, has_fix) {
        (0, _) if lowerable > 0 => o.next("spec check --write-baseline"),
        (0, _) => o.next("spec audit  # optional: stale, orphan and unfinished documents"),
        (_, true) => {
            o.code(Code::Findings).next(if a.staged { "spec fix --staged --write" } else { "spec fix --write" })
        }
        _ => o.code(Code::Findings).next("spec explain <rule>"),
    })
}
