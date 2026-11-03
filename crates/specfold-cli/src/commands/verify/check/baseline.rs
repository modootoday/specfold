//! The baseline side of `spec check`: recording today's errors, and the note on those it holds back.

use crate::cli::args::verify::CheckArgs;
use crate::output::Output;
use crate::session;
use anyhow::Result;
use specfold_core::check::{Finding, baseline};

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
        .next(format!("git add {}", specfold_core::shell::word(&rel))))
}

/// The summary's note on errors the baseline holds back.
pub(super) fn held_note(held: bool, known: usize) -> String {
    if held {
        format!(" · {known} more error(s) held by the baseline (--no-baseline shows them)")
    } else {
        String::new()
    }
}
