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
    let s = session::open(g)?;
    if a.translations {
        let (plan, n) = fix::translation_pairs(&s.corpus)?;
        let o = Output::new("fix")
            .field("fixable", n)
            .field("remaining", 0)
            .head(format!("{n} translation(s) paired by the layout get rel.translation_of and lang"));
        let o = preview::finish(o, &plan, &s.corpus.cfg.root, a.write, "spec fix --translations --write")?;
        return Ok(if a.write { o.next("spec check") } else { o });
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
    let mut rerun = String::from("spec fix --write");
    for r in &a.rules {
        rerun.push_str(&format!(" --rule {r}"));
    }
    let o = preview::finish(o, &plan, &s.corpus.cfg.root, a.write, &rerun)?;
    // With nothing to apply, `--write` would do nothing: point at the first finding left to
    // decide, or at a clean re-check.
    let undecided = (applied == 0).then(|| findings.first()).flatten();
    Ok(match undecided {
        _ if a.write => o.next("spec check"),
        Some(f) => o.next(format!("spec explain {}  # first decision: {}:{}", f.rule, f.path, f.line)),
        None if applied == 0 => o.next("spec check"),
        None => o,
    })
}
