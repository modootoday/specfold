//! One `spec check` run: the findings, the summary, the exit code and the next command.

use super::baseline::{held_note, write_baseline, write_ratchet};
use super::ci::{Ctx, ci_output};
use super::scope::{
    budget_measures, budget_out_of_scope, check_fail_on, check_rule_names, checked_range, documents_named,
    overlay_stdin, range_of, repo_paths,
};
use crate::cli::args::{global::Global, verify::CheckArgs};
use crate::commands::verify::owners;
use crate::output::{Code, Output, findings};
use crate::session;
use anyhow::Result;
use specfold_core::check::{self, Finding, Fix, Options, Selection, Severity, baseline};
use specfold_core::workspace::vcs;
use std::collections::BTreeMap;
use std::time::Instant;

fn failing(list: &[Finding], fail_on: &[String]) -> usize {
    if fail_on.iter().any(|r| r == "none") {
        return 0;
    }
    let counts = |f: &&Finding| fail_on.is_empty() || fail_on.iter().any(|r| r == "any" || r == &f.rule);
    list.iter().filter(|f| f.severity == Severity::Error).filter(counts).count()
}

/// The findings a run reports once the baseline holds known ones back and other owners widen it.
struct Held {
    list: Vec<Finding>,
    /// Errors the baseline knows; 0 without one.
    known: usize,
    /// Baseline entries that now hold fewer errors.
    lowerable: usize,
    baselined: bool,
    others: owners::Others,
}

/// Counts over the reported findings, shared by the summary, the text and the CI formats.
struct Tally {
    errors: usize,
    warnings: usize,
    fails: usize,
    cfg_warns: usize,
    ms: u128,
    excluded: BTreeMap<String, usize>,
    by_rule: BTreeMap<String, (usize, usize)>,
    checked: Option<(serde_json::Value, String)>,
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
    let paths = implied_paths(a);
    // With --path, standard input stands for one of those files, document or not.
    let stdin_doc = a.stdin_path.as_ref().filter(|_| paths.is_empty());
    let files: Vec<String> = a.files.iter().chain(stdin_doc).cloned().collect();
    // A full run reads what git tracks; files named on the command line are read as they are.
    let full = !a.staged && a.changed.is_none() && files.is_empty() && paths.is_empty();
    let untracked = a.include_untracked || std::env::var_os("SPECFOLD_INCLUDE_UNTRACKED").is_some_and(|v| v == "1");
    let s = if full && !untracked { session::open_tracked(g)? } else { session::open(g)? };
    let only = documents_named(&s, &files, stdin_doc.map(String::as_str))?;
    let named = repo_paths(&s, &paths);
    let commands = crate::cli::describe::commands().into_iter().map(|c| c.name).collect();
    let opts =
        Options { run_commands: a.run_commands, external: a.external, online: a.online, guards: a.guards, commands };
    let range = if named.is_empty() { range_of(a) } else { check::Range::Paths(named.clone()) };
    let mut report =
        check::run(&s.corpus, &s.graph, range, &opts, &Selection { only: a.rules.clone(), skip: a.skip.clone() })?;
    if a.write_baseline {
        return write_baseline(&s, a, &report.findings);
    }
    if a.write_ratchet {
        return write_ratchet(&s, a, &report);
    }
    let held = hold(&s, a, &mut report, &only, &opts)?;
    let (errors, warnings) = findings::counts(&held.list);
    let t = Tally {
        errors,
        warnings,
        fails: failing(&held.list, &a.fail_on),
        ms: started.elapsed().as_millis(),
        excluded: left_out(&held.list, &a.fail_on),
        // C119: config warnings (printed on stderr) count in the summary too.
        cfg_warns: session::config_warnings(&s.corpus.cfg).len(),
        by_rule: by_rule(&held.list),
        checked: checked_range(&s.corpus.cfg.spec_home, a),
    };
    let mut o = summary(&s, &report, &held, &t);
    if !named.is_empty() && report.rules_run.contains(&"B1") {
        o = o.field("budgets", budget_measures(&s, &named));
        let outside = budget_out_of_scope(&s, &named);
        if !outside.is_empty() {
            let json: Vec<_> = outside.iter().map(|(p, l)| serde_json::json!({"path": p, "gitlink": l})).collect();
            o = o.field("out_of_scope", json);
        }
        for (p, l) in &outside {
            o = o.foot(format!(
                "B1: {p} is inside the submodule {l} and no budget names it explicitly; out of scope, not judged"
            ));
        }
    }
    if a.format != "text" {
        let cx = Ctx {
            g,
            a,
            list: &held.list,
            errors: t.errors,
            warnings: t.warnings,
            fails: t.fails,
            root: &s.corpus.cfg.spec_home,
        };
        return ci_output(&cx, o);
    }
    Ok(text(&s, a, o, &report, &held, &t))
}

/// `--path`, or the `--stdin-path` file when only B1 runs: a size budget judges any file, so
/// `--rule B1 --stdin-path AGENTS.md` means `--path AGENTS.md` rather than a document.
fn implied_paths(a: &CheckArgs) -> Vec<String> {
    let only_b1 = !a.rules.is_empty() && a.rules.iter().all(|r| r == "B1");
    let unscoped = a.files.is_empty() && !a.staged && a.changed.is_none();
    match &a.stdin_path {
        Some(p) if a.paths.is_empty() && only_b1 && unscoped => vec![p.clone()],
        _ => a.paths.clone(),
    }
}

/// Holds known errors back against the baseline, keeps the named files, and adds what other
/// owners' runs report.
fn hold(
    s: &session::Session,
    a: &CheckArgs,
    report: &mut check::Report,
    only: &[String],
    opts: &Options,
) -> Result<Held> {
    let t = std::time::Instant::now();
    let base = if a.no_baseline { None } else { baseline::load(&s.corpus)? };
    let (list, known, lowerable) = match &base {
        Some(b) => {
            let held = baseline::hold(&s.corpus, b, std::mem::take(&mut report.findings));
            let lowerable = held.lowerable.iter().filter(|(k, _, _)| report.judged(k)).count();
            (held.findings, held.known, lowerable)
        }
        None => (std::mem::take(&mut report.findings), 0, 0),
    };
    let t = specfold_core::trace::phase("baseline", t);
    let list = if only.is_empty() { list } else { list.into_iter().filter(|f| only.contains(&f.path)).collect() };
    let (list, others) = owners::widen(s, a, list, base.as_ref(), opts)?;
    specfold_core::trace::phase("other-owners", t);
    Ok(Held { list, known, lowerable, baselined: base.is_some(), others })
}

/// The summary fields every format carries.
fn summary(s: &session::Session, report: &check::Report, held: &Held, t: &Tally) -> Output {
    let mut o = Output::new("check")
        .field("errors", t.errors)
        .field("warnings", t.warnings)
        .field("config_warnings", t.cfg_warns)
        .field("docs_checked", report.docs_checked)
        .field("docs_total", s.corpus.docs.len())
        .field("rules", report.rules_run.clone())
        .field("elapsed_ms", t.ms as u64);
    o = t.checked.iter().fold(o, |o, (json, _)| o.field("range", json.clone()));
    if !t.excluded.is_empty() {
        o = o.field(
            "not_failing",
            t.excluded.iter().map(|(r, n)| (r.clone(), serde_json::json!(n))).collect::<serde_json::Map<_, _>>(),
        );
    }
    if held.baselined {
        o = o.field("baselined", held.known).field("lowerable", held.lowerable);
    }
    if !report.gitlinks.is_empty() {
        o = o.field("gitlinks", super::gitlinks::json(&report.gitlinks));
    }
    if let Some(j) = &report.ratchet {
        o = o.field(
            "ratchet",
            serde_json::json!({"rose": j.rises.len(), "fell": j.falls.len(), "unrecorded": j.unrecorded}),
        );
    }
    o.field(
        "by_rule",
        t.by_rule
            .iter()
            .map(|(r, (e, w))| (r.clone(), serde_json::json!({"errors": e, "warnings": w})))
            .collect::<serde_json::Map<_, _>>(),
    )
}

/// The text output: the findings, the totals line and its notes, and the next command.
fn text(s: &session::Session, a: &CheckArgs, o: Output, report: &check::Report, held: &Held, t: &Tally) -> Output {
    let held_line = held_note(held.baselined && held.known > 0, held.known);
    let (errors, warnings, cfg_warns) = (t.errors, t.warnings, t.cfg_warns);
    let o = list_items(o, a, &t.by_rule, &held.list);
    // A staged or ranged run that changes no document still checks the changed code (D1, U1).
    let place = if report.docs_checked == 0 && (a.staged || a.changed.is_some()) {
        "in the changed files (no document changed)".to_string()
    } else {
        format!("in {} of {} documents", report.docs_checked, s.corpus.docs.len())
    };
    let in_cfg = if cfg_warns > 0 { format!(" (+{cfg_warns} in spec.toml)") } else { String::new() };
    let o = o.foot(format!(
        "{errors} error(s), {warnings} warning(s){in_cfg} {place}{held_line} · {:.2}s",
        t.ms as f64 / 1000.0
    ));
    let o = t.checked.iter().fold(o, |o, (_, line)| o.foot(line.clone()));
    let o = super::gitlinks::lines(&report.gitlinks).into_iter().fold(o, |o, l| o.foot(l));
    let fell = report.ratchet.as_ref().map_or(0, |j| j.falls.len());
    let o = if fell > 0 {
        o.foot(format!("W1: {fell} ratcheted count(s) fell; spec check --write-ratchet --yes records them"))
    } else {
        o
    };
    let o = owners::annotate(o, &s.corpus.cfg.spec_home, &held.others);
    let mut o = t
        .excluded
        .iter()
        .fold(o, |o, (r, n)| o.foot(format!("{r}: {n} error(s) left out of the exit code by --fail-on")));
    if a.new {
        o = new_docs(s, o);
    }
    next_step(s, a, o, held, t)
}

/// The next command: the baseline to lower, the rule left out, the warnings or errors to fix,
/// or an optional audit when all is well.
fn next_step(s: &session::Session, a: &CheckArgs, o: Output, held: &Held, t: &Tally) -> Output {
    let list = &held.list;
    let (has_fix, fix_next) = fix_next(s, a, list);
    let lowerable = held.lowerable;
    match (t.fails, has_fix) {
        // D2: optional; rewriting only lowers the counts, nothing is wrong.
        (0, _) if lowerable > 0 => o.next(format!(
            "spec check --write-baseline  # optional: only lowers the baseline, {lowerable} entr(ies) now hold fewer errors"
        )),
        (0, _) if !t.excluded.is_empty() => {
            let (rule, n) = t.excluded.iter().next().map(|(r, n)| (r.clone(), *n)).unwrap_or_default();
            o.next(format!("spec explain {rule}  # {n} error(s) left out of the exit code"))
        }
        (0, _) if t.warnings > 0 => {
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
    }
}

/// Errors per rule that --fail-on leaves out of the exit code.
fn left_out(list: &[Finding], fail_on: &[String]) -> BTreeMap<String, usize> {
    let mut excluded: BTreeMap<String, usize> = Default::default();
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
        vcs::committed(&s.corpus.cfg.spec_home).unwrap_or_default().into_iter().collect();
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
fn by_rule(list: &[Finding]) -> BTreeMap<String, (usize, usize)> {
    let mut by_rule: BTreeMap<String, (usize, usize)> = Default::default();
    for f in list {
        let e = by_rule.entry(f.rule.clone()).or_default();
        if f.severity == Severity::Error { e.0 += 1 } else { e.1 += 1 }
    }
    by_rule
}

/// The items of a text run: per-rule counts, errors only, or every finding.
fn list_items(o: Output, a: &CheckArgs, by_rule: &BTreeMap<String, (usize, usize)>, list: &[Finding]) -> Output {
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
