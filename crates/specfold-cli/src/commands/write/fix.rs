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
        let (o, written) = preview::finish_bounded(
            o,
            &plan,
            &s.corpus.cfg.root,
            max,
            a.write,
            a.yes,
            "spec fix --translations --write",
        )?;
        return Ok(if written { o.next("spec check") } else { o });
    }
    let range = if a.staged { Range::Staged } else { Range::All };
    let sel = Selection { only: a.rules.clone(), skip: vec![] };
    let report = check::run(&s.corpus, &s.graph, range, &Options::default(), &sel)?;
    let targets: Vec<(String, Option<usize>)> =
        a.targets.iter().map(|t| split_target(t)).map(|(p, l)| (session::repo_path(&s, &p), l)).collect();
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
        rerun.push_str(&format!(" --rule {}", session::shell_word(r)));
    }
    if let Some(id) = &a.pick {
        rerun.push_str(&format!(" --pick {}", session::shell_word(id)));
    }
    for t in &a.targets {
        rerun.push_str(&format!(" {}", session::shell_word(t)));
    }
    let max = s.corpus.cfg.raw.write.max_files;
    let (o, written) = preview::finish_bounded(o, &plan, &s.corpus.cfg.root, max, a.write, a.yes, &rerun)?;
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
