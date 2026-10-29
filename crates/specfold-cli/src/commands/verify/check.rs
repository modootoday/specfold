//! `spec check`.

use crate::cli::args::{global::Global, verify::CheckArgs};
use crate::output::{Code, Output, ci, findings};
use crate::session;
use anyhow::{Result, bail};
use specfold_core::check::{self, Finding, Options, Range, Selection, Severity, baseline};
use specfold_core::workspace::vcs;
use std::time::Instant;

/// `--fail-on` must name rules that exist, so a gate cannot silently wait on a rule that never fires.
fn check_fail_on(names: &[String]) -> Result<()> {
    let known: Vec<&str> = check::rules::catalog().iter().map(|r| r.info().id).collect();
    for n in names {
        if n != "any" && n != "none" && !n.starts_with("x.") && !known.contains(&n.as_str()) {
            bail!("--fail-on {n}: no such rule (spec explain lists them)");
        }
    }
    Ok(())
}

fn failing(list: &[Finding], fail_on: &[String]) -> usize {
    if fail_on.iter().any(|r| r == "none") {
        return 0;
    }
    let counts = |f: &&Finding| fail_on.is_empty() || fail_on.iter().any(|r| r == "any" || r == &f.rule);
    list.iter().filter(|f| f.severity == Severity::Error).filter(counts).count()
}

/// What `spec check <FILE>` lists for one file, baseline applied: the language server's source.
pub fn file_findings(g: &Global, rel: &str) -> Result<Vec<Finding>> {
    Ok(all_findings(g)?.into_iter().filter(|f| f.path == rel).collect())
}

/// Every finding of a full check (no commands, no network), with the baseline applied.
pub fn all_findings(g: &Global) -> Result<Vec<Finding>> {
    let s = session::open(g)?;
    let commands = crate::cli::describe::commands().into_iter().map(|c| c.name).collect();
    let opts = Options { run_commands: false, external: false, online: false, guards: false, commands };
    let report = check::run(&s.corpus, &s.graph, Range::All, &opts, &Selection { only: vec![], skip: vec![] })?;
    let list = match baseline::load(&s.corpus)? {
        Some(b) => baseline::hold(&s.corpus, &b, report.findings).findings,
        None => report.findings,
    };
    Ok(list)
}

/// An absolute path for a file that may not exist yet: the nearest existing ancestor is resolved.
pub fn absolute(p: &std::path::Path) -> std::path::PathBuf {
    let p = if p.is_absolute() { p.to_path_buf() } else { std::env::current_dir().unwrap_or_default().join(p) };
    let mut rest = vec![];
    let mut cur = p.as_path();
    loop {
        if let Ok(c) = cur.canonicalize() {
            return rest.iter().rev().fold(c, |acc, part| acc.join(part));
        }
        match (cur.parent(), cur.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name.to_os_string());
                cur = parent;
            }
            _ => return p,
        }
    }
}

/// The files named on the command line, repository-relative; each must be a document file.
fn documents_named(s: &session::Session, files: &[String], stdin: Option<&str>) -> Result<Vec<String>> {
    let mut out = vec![];
    for f in files {
        if f == "-" {
            match stdin {
                Some(_) => continue,
                None => bail!(
                    "-: spec check reads standard input only for a named path; next: spec check --stdin-path <FILE>  # text on stdin checked as <FILE>"
                ),
            }
        }
        let abs = absolute(std::path::Path::new(f));
        let rel = match abs.strip_prefix(&s.corpus.cfg.root) {
            Ok(r) => r.to_string_lossy().replace('\\', "/"),
            Err(_) => session::repo_path(s, f),
        };
        let known = s.corpus.docs.iter().any(|d| d.path == rel || d.chapters.iter().any(|c| c == &rel));
        if !known && !std::path::Path::new(f).exists() {
            if let Some(d) = s.corpus.docs.iter().find(|d| d.id.to_string() == *f) {
                out.push(d.path.clone());
                continue;
            }
            if stdin == Some(f.as_str()) {
                let parent = std::path::Path::new(&rel).parent().map(|p| p.to_string_lossy().replace('\\', "/"));
                let sibling = s.corpus.docs.iter().find(|d| {
                    std::path::Path::new(&d.path).parent().map(|p| p.to_string_lossy().replace('\\', "/")) == parent
                });
                match sibling {
                    Some(d) => {
                        let kind = d.id.to_string().split(':').next().unwrap_or("").to_string();
                        bail!(
                            "{f}: the name does not follow this directory's document naming (like {}), so it is not a document; next: spec new {kind} --title <TITLE> --dry-run  # prints a path to use",
                            d.path
                        )
                    }
                    None => bail!(
                        "{f}: not in a spec directory, so the buffer is not a document; next: spec doctor  # lists the spec directories"
                    ),
                }
            }
            bail!("{f}: no such file or document id (spec check takes document files or ids); next: spec resolve {f}");
        }
        if !known {
            bail!(
                "{f}: not a document (spec check <FILE> takes a document file; `spec doctor` lists files it skipped)"
            );
        }
        out.push(rel);
    }
    Ok(out)
}

pub fn run(g: &Global, a: &CheckArgs) -> Result<Output> {
    let started = Instant::now();
    check_fail_on(&a.fail_on)?;
    if let Some(p) = &a.stdin_path {
        let abs = absolute(std::path::Path::new(p));
        let mut text = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut text)?;
        specfold_core::corpus::overlay::set(abs, text);
    }
    let s = session::open(g)?;
    let files: Vec<String> = a.files.iter().chain(&a.stdin_path).cloned().collect();
    let only = documents_named(&s, &files, a.stdin_path.as_deref())?;
    let range = match (&a.changed, a.staged) {
        (Some(base), _) => Range::Changed(base.clone()),
        (None, true) => Range::Staged,
        (None, false) => Range::All,
    };
    let commands = crate::cli::describe::commands().into_iter().map(|c| c.name).collect();
    let opts =
        Options { run_commands: a.run_commands, external: a.external, online: a.online, guards: a.guards, commands };
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
    let list = if only.is_empty() { list } else { list.into_iter().filter(|f| only.contains(&f.path)).collect() };
    let (errors, warnings) = findings::counts(&list);
    let fails = failing(&list, &a.fail_on);
    let ms = started.elapsed().as_millis();
    let mut excluded: std::collections::BTreeMap<String, usize> = Default::default();
    if !a.fail_on.is_empty() && !a.fail_on.iter().any(|r| r == "any") {
        let none = a.fail_on.iter().any(|r| r == "none");
        for f in list.iter().filter(|f| f.severity == Severity::Error) {
            if none || !a.fail_on.contains(&f.rule) {
                *excluded.entry(f.rule.clone()).or_default() += 1;
            }
        }
    }
    let mut o = Output::new("check")
        .field("errors", errors)
        .field("warnings", warnings)
        .field("docs_checked", report.docs_checked)
        .field("docs_total", s.corpus.docs.len())
        .field("rules", report.rules_run.clone())
        .field("elapsed_ms", ms as u64);
    if !excluded.is_empty() {
        o = o.field(
            "not_failing",
            excluded.iter().map(|(r, n)| (r.clone(), serde_json::json!(n))).collect::<serde_json::Map<_, _>>(),
        );
    }
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
    if a.format != "text" {
        return ci_output(g, a, o, &list, errors, warnings, fails, &s.corpus.cfg.root);
    }
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
    let mut o = excluded
        .iter()
        .fold(o, |o, (r, n)| o.foot(format!("{r}: {n} error(s) left out of the exit code by --fail-on")));
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
        (0, _) if !excluded.is_empty() => {
            let (rule, n) = excluded.iter().next().map(|(r, n)| (r.clone(), *n)).unwrap_or_default();
            o.next(format!("spec explain {rule}  # {n} error(s) left out of the exit code"))
        }
        (0, _) if warnings > 0 => {
            // Point at the warnings before the optional audit: an autofix when one exists,
            // otherwise the first warning's rule.
            let w: Vec<_> = list.iter().filter(|f| f.severity != Severity::Error).collect();
            if w.iter().any(|f| f.fix.is_some()) {
                o.next(if a.staged { "spec fix --staged --write" } else { "spec fix --write" })
            } else {
                o.next(format!("spec explain {}  # {} warning(s) to fix", w[0].rule, w.len()))
            }
        }
        (0, _) => o.next("spec audit  # optional: stale, orphan and unfinished documents"),
        (_, true) => {
            o.code(Code::Findings).next(if a.staged { "spec fix --staged --write" } else { "spec fix --write" })
        }
        _ => {
            let rule = list.iter().find(|f| f.severity == Severity::Error).map(|f| f.rule.clone()).unwrap_or_default();
            o.code(Code::Findings).next(format!("spec explain {rule}"))
        }
    })
}

/// A CI format replaces the text: only the document goes to standard output, so it can be
/// redirected to a file, and the exit code still follows --fail-on.
#[allow(clippy::too_many_arguments)]
fn ci_output(
    g: &Global,
    a: &CheckArgs,
    o: Output,
    list: &[Finding],
    errors: usize,
    warnings: usize,
    fails: usize,
    root: &std::path::Path,
) -> Result<Output> {
    if g.json || g.jsonl {
        bail!("--format {} is its own output; drop --json/--jsonl", a.format);
    }
    let shown: Vec<Finding> =
        list.iter().filter(|f| !a.errors_only || f.severity == Severity::Error).cloned().collect();
    let doc = match a.format.as_str() {
        "github" => {
            if let Ok(path) = std::env::var("GITHUB_STEP_SUMMARY")
                && !path.is_empty()
            {
                use std::io::Write;
                if let Ok(mut f) = std::fs::OpenOptions::new().append(true).create(true).open(&path) {
                    let _ = f.write_all(ci::github_summary(&shown, errors, warnings).as_bytes());
                }
            }
            ci::github(&shown, errors, warnings)
        }
        "sarif" => ci::sarif(&shown),
        "gitlab" => ci::gitlab(&shown),
        "editor" => ci::editor(&shown),
        _ => ci::rdjson(&shown, root),
    };
    let mut out = Output::new("check").unpaged();
    out.summary = o.summary;
    let out = doc.trim_end_matches('\n').lines().fold(out, |o, l| o.head(l));
    Ok(if fails > 0 { out.code(Code::Findings) } else { out })
}
