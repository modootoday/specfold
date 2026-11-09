//! The baseline side of `spec check`: recording today's errors, the note on those it holds back,
//! and lowering the `[ratchet]` record of warning counts.

use crate::cli::args::verify::CheckArgs;
use crate::output::Output;
use crate::session;
use anyhow::Result;
use specfold_core::check::limits::ratchet;
use specfold_core::check::{Finding, Report, baseline};

/// `--write-ratchet`: preview, or with --yes write, the record lowered to this full run.
pub(super) fn write_ratchet(s: &session::Session, a: &CheckArgs, report: &Report) -> Result<Output> {
    let (Some(j), Some(_)) = (&report.ratchet, &s.corpus.cfg.raw.ratchet.file) else {
        return Ok(Output::error(
            "check",
            crate::output::Code::Usage,
            "--write-ratchet needs [ratchet] rules and file in spec.toml, a readable record, and a full run of those rules",
            Some("spec explain W1".into()),
        )
        .field("written", false));
    };
    let current = ratchet::load(&s.corpus)?;
    let next = ratchet::lowered(current.as_ref(), j);
    let changes = j.falls.len() + j.unrecorded.len();
    let o = Output::new("check").field("fell", j.falls.len()).field("unrecorded", j.unrecorded.clone());
    if changes == 0 {
        return Ok(o.field("written", false).head("the ratchet record already holds this run: nothing to write"));
    }
    if !a.yes {
        return Ok(o
            .field("written", false)
            .head(format!("would record {changes} change(s) in the ratchet record: nothing was written"))
            .next("spec check --write-ratchet --yes"));
    }
    let rel = ratchet::write(&s.corpus, &next)?;
    Ok(o.field("written", true)
        .field("ratchet", rel.clone())
        .foot(format!("{changes} change(s) recorded in {rel}"))
        .next(format!("git add {}", specfold_core::suggest::shell::word(&rel))))
}

/// `--write-baseline`: preview, or with --yes record, today's errors as the baseline.
pub(super) fn write_baseline(s: &session::Session, a: &CheckArgs, findings: &[Finding]) -> Result<Output> {
    // C102: without a baseline path there is nothing to preview; say what to set first.
    if s.corpus.cfg.raw.check.baseline.is_none() {
        // C134: JSON carries the reason as `error`, like every other rc 2.
        return Ok(Output::error(
            "check",
            crate::output::Code::Usage,
            "--write-baseline needs a file to write: add `[check]` with `baseline = \".spec-baseline.json\"` to spec.toml",
            Some("spec explain config".into()),
        )
        .field("written", false));
    }
    let b = baseline::measure(&s.corpus, findings)?;
    let total: usize = b.counts.values().sum();
    // A baseline hides today's errors from every later run: --yes confirms it (CLI-3).
    if !a.yes {
        return Ok(Output::new("check")
            .field("written", false)
            .field("baselined", total)
            .head(format!(
                "would record {total} error(s) in {} rule/path pair(s) as the baseline, hiding them from later runs: nothing was written",
                b.counts.len()
            ))
            .next("spec check --write-baseline --yes"));
    }
    let rel = baseline::write(&s.corpus, &b)?;
    Ok(Output::new("check")
        .field("baseline", rel.clone())
        .field("measured_on", b.measured_on.clone())
        .field("baselined", total)
        .foot(format!("{total} error(s) in {} rule/path pair(s) recorded in {rel}", b.counts.len()))
        .next(format!("git add {}", specfold_core::suggest::shell::word(&rel))))
}

/// The summary's note on errors the baseline holds back.
pub(super) fn held_note(held: bool, known: usize) -> String {
    if held {
        format!(" · {known} more error(s) held by the baseline (--no-baseline shows them)")
    } else {
        String::new()
    }
}
