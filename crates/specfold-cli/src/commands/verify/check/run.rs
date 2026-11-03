//! One `spec check` run: the findings, the summary, the exit code and the next command.

use super::baseline::{held_note, write_baseline};
use super::ci::ci_output;
use super::scope::{check_fail_on, check_rule_names, checked_range, documents_named, overlay_stdin, range_of};
use crate::cli::args::{global::Global, verify::CheckArgs};
use crate::commands::verify::owners;
use crate::output::{Code, Output, findings};
use crate::session;
use anyhow::Result;
use specfold_core::check::{self, Finding, Fix, Options, Selection, Severity, baseline};
use specfold_core::workspace::vcs;
use std::time::Instant;

fn failing(list: &[Finding], fail_on: &[String]) -> usize {
    if fail_on.iter().any(|r| r == "none") {
        return 0;
    }
    let counts = |f: &&Finding| fail_on.is_empty() || fail_on.iter().any(|r| r == "any" || r == &f.rule);
    list.iter().filter(|f| f.severity == Severity::Error).filter(counts).count()
}

pub fn run(g: &Global, a: &CheckArgs) -> Result<Output> {
    let started = Instant::now();
    check_fail_on(&a.fail_on)?;
    check_rule_names("--rule", &a.rules)?;
    check_rule_names("--skip", &a.skip)?;
    if let Some(p) = &a.stdin_path {
        overlay_stdin(&session::location(g), p)?;
    }
    // C116: config warnings are a line each and not findings, so -q shows them too.
    session::show_config_warnings();
    let s = session::open(g)?;
    let files: Vec<String> = a.files.iter().chain(&a.stdin_path).cloned().collect();
    let only = documents_named(&s, &files, a.stdin_path.as_deref())?;
    let commands = crate::cli::describe::commands().into_iter().map(|c| c.name).collect();
    let opts =
        Options { run_commands: a.run_commands, external: a.external, online: a.online, guards: a.guards, commands };
    let mut report = check::run(
        &s.corpus,
        &s.graph,
        range_of(a),
        &opts,
        &Selection { only: a.rules.clone(), skip: a.skip.clone() },
    )?;
    if a.write_baseline {
        return write_baseline(&s, a, &report.findings);
    }
    let t = std::time::Instant::now();
    let base = if a.no_baseline { None } else { baseline::load(&s.corpus)? };
    let (list, known, lowerable) = match &base {
        Some(b) => {
            let held = baseline::hold(&s.corpus, b, std::mem::take(&mut report.findings));
            let lowerable = held.lowerable.iter().filter(|(k, _, _)| report.judged(k)).count();
            (held.findings, held.known, lowerable)
        }
        None => (report.findings, 0, 0),
    };
    let t = specfold_core::trace::phase("baseline", t);
    let list = if only.is_empty() { list } else { list.into_iter().filter(|f| only.contains(&f.path)).collect() };
    let (list, others) = owners::widen(&s, a, list, base.as_ref(), &opts)?;
    specfold_core::trace::phase("other-owners", t);
    let (errors, warnings) = findings::counts(&list);
    let fails = failing(&list, &a.fail_on);
    let ms = started.elapsed().as_millis();
    let excluded = left_out(&list, &a.fail_on);
    // C119: config warnings (printed on stderr) count in the summary too.
    let cfg_warns = session::config_warnings(&s.corpus.cfg).len();
    let mut o = Output::new("check")
        .field("errors", errors)
        .field("warnings", warnings)
        .field("config_warnings", cfg_warns)
        .field("docs_checked", report.docs_checked)
        .field("docs_total", s.corpus.docs.len())
        .field("rules", report.rules_run.clone())
        .field("elapsed_ms", ms as u64);
    let checked = checked_range(&s.corpus.cfg.root, a);
    o = checked.iter().fold(o, |o, (json, _)| o.field("range", json.clone()));
    if !excluded.is_empty() {
        o = o.field(
            "not_failing",
            excluded.iter().map(|(r, n)| (r.clone(), serde_json::json!(n))).collect::<serde_json::Map<_, _>>(),
        );
    }
    if base.is_some() {
        o = o.field("baselined", known).field("lowerable", lowerable);
    }
    let by_rule = by_rule(&list);
    o = o.field(
        "by_rule",
        by_rule
            .iter()
            .map(|(r, (e, w))| (r.clone(), serde_json::json!({"errors": e, "warnings": w})))
            .collect::<serde_json::Map<_, _>>(),
    );
    let held = held_note(base.is_some() && known > 0, known);
    if a.format != "text" {
        return ci_output(g, a, o, &list, errors, warnings, fails, &s.corpus.cfg.root);
    }
    let o = list_items(o, a, &by_rule, &list);
    // A staged or ranged run that changes no document still checks the changed code (D1, U1).
    let place = if report.docs_checked == 0 && (a.staged || a.changed.is_some()) {
        "in the changed files (no document changed)".to_string()
    } else {
        format!("in {} of {} documents", report.docs_checked, s.corpus.docs.len())
    };
    let in_cfg = if cfg_warns > 0 { format!(" (+{cfg_warns} in spec.toml)") } else { String::new() };
    let o =
        o.foot(format!("{errors} error(s), {warnings} warning(s){in_cfg} {place}{held} · {:.2}s", ms as f64 / 1000.0));
    let o = checked.iter().fold(o, |o, (_, line)| o.foot(line.clone()));
    let o = owners::annotate(o, &s.corpus.cfg.root, &others);
    let mut o = excluded
        .iter()
        .fold(o, |o, (r, n)| o.foot(format!("{r}: {n} error(s) left out of the exit code by --fail-on")));
    if a.new {
        o = new_docs(&s, o);
    }
    let (has_fix, fix_next) = fix_next(&s, a, &list);
    Ok(match (fails, has_fix) {
        // D2: optional; rewriting only lowers the counts, nothing is wrong.
        (0, _) if lowerable > 0 => o.next(format!(
            "spec check --write-baseline  # optional: only lowers the baseline, {lowerable} entr(ies) now hold fewer errors"
        )),
        (0, _) if !excluded.is_empty() => {
            let (rule, n) = excluded.iter().next().map(|(r, n)| (r.clone(), *n)).unwrap_or_default();
            o.next(format!("spec explain {rule}  # {n} error(s) left out of the exit code"))
        }
        (0, _) if warnings > 0 => {
            // Point at the warnings before the optional audit: an autofix when one exists,
            // otherwise the first warning's rule.
            let w: Vec<_> = list.iter().filter(|f| f.severity != Severity::Error).collect();
            if w.iter().any(|f| matches!(f.fix, Some(Fix::Replace { .. }))) {
                o.next(fix_next)
            } else if let Some(Fix::Cmd(c)) = &w[0].fix {
                // A command to finish by hand, such as the section a type requires.
                o.next(format!("{c}  # {} warning(s) to fix", w.len()))
            } else {
                o.next(format!("spec explain {}  # {} warning(s) to fix", w[0].rule, w.len()))
            }
        }
        (0, _) => o.next("spec audit  # optional: stale, orphan and unfinished documents"),
        (_, true) => o.code(Code::Findings).next(fix_next),
        _ => {
            let rule = list.iter().find(|f| f.severity == Severity::Error).map(|f| f.rule.clone()).unwrap_or_default();
            o.code(Code::Findings).next(format!("spec explain {rule}"))
        }
    })
}

/// Errors per rule that --fail-on leaves out of the exit code.
fn left_out(list: &[Finding], fail_on: &[String]) -> std::collections::BTreeMap<String, usize> {
    let mut excluded: std::collections::BTreeMap<String, usize> = Default::default();
    if !fail_on.is_empty() && !fail_on.iter().any(|r| r == "any") {
        let none = fail_on.iter().any(|r| r == "none");
        for f in list.iter().filter(|f| f.severity == Severity::Error) {
            if none || !fail_on.contains(&f.rule) {
                *excluded.entry(f.rule.clone()).or_default() += 1;
            }
        }
    }
    excluded
}

/// `--new`: the documents not in the last commit.
fn new_docs(s: &session::Session, mut o: Output) -> Output {
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
    o
}

/// Whether `spec fix` applies any finding, and the command that applies them.
fn fix_next(s: &session::Session, a: &CheckArgs, list: &[Finding]) -> (bool, String) {
    // Only fixes `spec fix` applies; a command to finish by hand is shown with its finding.
    let has_fix = list.iter().any(|f| {
        matches!(&f.fix, Some(Fix::Replace { .. })) || matches!(&f.fix, Some(Fix::Cmd(c)) if c.starts_with("spec fix"))
    });
    // C112: when the fixes touch more files than [write] max_files, `spec fix --write` alone
    // would refuse, so the next says --yes.
    let fixable: std::collections::BTreeSet<&str> = list
        .iter()
        .filter(|f| {
            matches!(&f.fix, Some(Fix::Replace { .. }))
                || matches!(&f.fix, Some(Fix::Cmd(c)) if c.starts_with("spec fix"))
        })
        .map(|f| f.path.as_str())
        .collect();
    let max = s.corpus.cfg.raw.write.max_files;
    let fix_next = format!(
        "spec fix{} --write{}",
        if a.staged { " --staged" } else { "" },
        if max > 0 && fixable.len() > max { " --yes" } else { "" }
    );
    (has_fix, fix_next)
}

/// (errors, warnings) per rule.
fn by_rule(list: &[Finding]) -> std::collections::BTreeMap<String, (usize, usize)> {
    let mut by_rule: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
    for f in list {
        let e = by_rule.entry(f.rule.clone()).or_default();
        if f.severity == Severity::Error { e.0 += 1 } else { e.1 += 1 }
    }
    by_rule
}

/// The items of a text run: per-rule counts, errors only, or every finding.
fn list_items(
    o: Output,
    a: &CheckArgs,
    by_rule: &std::collections::BTreeMap<String, (usize, usize)>,
    list: &[Finding],
) -> Output {
    if a.summary {
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
        findings::push_all(o, list)
    }
}
