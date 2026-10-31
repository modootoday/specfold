//! `spec fix`: runs the rules, then applies the fixes that need no judgement.

use super::preview;
use crate::cli::args::{global::Global, write::FixArgs};
use crate::output::Output;
use crate::session;
use anyhow::{Result, bail};
use specfold_core::authoring::fix::{self, Pick};
use specfold_core::check::{self, Options, Range, Selection};

fn split_target(t: &str) -> (String, Option<usize>) {
    match t.rsplit_once(':') {
        Some((p, l)) if l.parse::<usize>().is_ok() => (p.to_string(), l.parse().ok()),
        _ => (t.to_string(), None),
    }
}

/// C214: each target is checked the way `spec check` checks its files (an existing document,
/// an id, `:N` a line number), so a typo is refused instead of reading as nothing to fix.
fn resolve_targets(s: &session::Session, targets: &[String]) -> Result<Vec<(String, Option<usize>)>> {
    let mut out = vec![];
    for t in targets {
        let (p, l) = split_target(t);
        if l.is_none()
            && let Some((head, tail)) = t.rsplit_once(':')
            && head.ends_with(".md")
            && !std::path::Path::new(t).exists()
        {
            bail!("{t}: '{tail}' is not a line number; use <FILE>:<LINE>, e.g. {head}:1; next: spec check {head}");
        }
        let paths = crate::commands::verify::check::documents_named(s, std::slice::from_ref(&p), None)?;
        out.extend(paths.into_iter().map(|p| (p, l)));
    }
    Ok(out)
}

/// C168: `--write` with nothing to fix, as `mv` and `set` (C164): nothing written, no undo line.
fn no_change(o: Output) -> Output {
    o.field("written", false).field("changed", false).foot("no change: nothing to fix")
}

pub fn run(g: &Global, a: &FixArgs) -> Result<Output> {
    // C167: the same check as `spec check --rule`, so a typo cannot read as nothing to fix.
    crate::commands::verify::check::check_rule_names("--rule", &a.rules)?;
    let s = session::open(g)?;
    if a.translations {
        let (plan, n) = fix::translation_pairs(&s.corpus)?;
        let o = Output::new("fix")
            .field("fixable", n)
            .field("remaining", 0)
            .head(format!("{n} translation(s) paired by the layout get rel.translation_of and lang"));
        let max = s.corpus.cfg.raw.write.max_files;
        let (o, written) = if a.write && plan.changes.is_empty() {
            (no_change(o), false)
        } else {
            let rerun = "spec fix --translations --write";
            preview::finish_bounded(o, &plan, &s.corpus.cfg.root, max, a.write, a.yes, rerun)?
        };
        return Ok(if written { o.next("spec check") } else { o });
    }
    let range = if a.staged { Range::Staged } else { Range::All };
    let sel = Selection { only: a.rules.clone(), skip: vec![] };
    let report = check::run(&s.corpus, &s.graph, range, &Options::default(), &sel)?;
    let targets = resolve_targets(&s, &a.targets)?;
    let findings: Vec<_> = report
        .findings
        .into_iter()
        .filter(|f| targets.is_empty() || targets.iter().any(|(p, l)| &f.path == p && l.is_none_or(|n| n == f.line)))
        .collect();
    let pick = match &a.pick {
        Some(id) => {
            let [(path, Some(line))] = targets.as_slice() else { bail!("--pick needs exactly one path:line target") };
            // C146: a pick must be one of the choices the finding offers.
            let offered: Vec<&String> = findings
                .iter()
                .filter(|f| &f.path == path && (f.line == *line || f.rule == "I2"))
                .flat_map(|f| f.candidates.iter())
                .collect();
            if !offered.contains(&id) {
                if offered.is_empty() {
                    bail!("{path}:{line} has no finding with choices to pick from; see spec check {path}");
                }
                let list: Vec<&str> = offered.iter().map(|s| s.as_str()).collect();
                bail!("'{id}' is not a choice at {path}:{line}; pick one of: {}", list.join(", "));
            }
            Some(Pick { id: id.clone(), path: path.clone(), line: *line })
        }
        None => None,
    };
    let (plan, applied) = fix::plan(&s.corpus, &findings, pick.as_ref())?;
    let left = findings.len().saturating_sub(applied);
    let o = Output::new("fix")
        .field("fixable", applied)
        .field("remaining", left)
        .head(format!("{applied} fix(es) ready, {left} finding(s) need a decision"));
    // A file that is not UTF-8 has nothing to fill in; say where it breaks, as check does.
    let mut o = o;
    for f in findings.iter().filter(|f| f.message.contains(specfold_core::corpus::overlay::UTF8)) {
        o = o.head(format!("{}:{}: {}", f.path, f.line, f.message));
    }
    // C145: the rerun repeats the scope it previewed: staged, rules, pick and targets.
    let mut rerun = String::from("spec fix --write");
    if a.staged {
        rerun.push_str(" --staged");
    }
    for r in &a.rules {
        rerun.push_str(&format!(" {}", specfold_core::shell::opt("--rule", r)));
    }
    if let Some(id) = &a.pick {
        rerun.push_str(&format!(" {}", specfold_core::shell::opt("--pick", id)));
    }
    for t in &a.targets {
        rerun.push_str(&format!(" {}", specfold_core::shell::quote(t)));
    }
    let max = s.corpus.cfg.raw.write.max_files;
    let (o, written) = if a.write && plan.changes.is_empty() {
        (no_change(o), false)
    } else {
        preview::finish_bounded(o, &plan, &s.corpus.cfg.root, max, a.write, a.yes, &rerun)?
    };
    if o.summary.contains_key("bounded") {
        return Ok(o);
    }
    // With nothing to apply, `--write` would do nothing: point at the first finding left to
    // decide, or at a clean re-check.
    let undecided = (applied == 0).then(|| findings.first()).flatten();
    Ok(match undecided {
        _ if written => o.next("spec check"),
        Some(f) => o.next(format!("spec explain {}  # first decision: {}:{}", f.rule, f.path, f.line)),
        None if applied == 0 => o.next("spec check"),
        None => o,
    })
}
