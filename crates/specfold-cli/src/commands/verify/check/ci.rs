//! `spec check --format`: a CI document in place of the text output.

use super::scope::checked_range;
use crate::cli::args::{global::Global, verify::CheckArgs};
use crate::output::{Code, Output, ci};
use anyhow::{Result, bail};
use specfold_core::check::{Finding, Severity};

/// What a CI document is made from: the flags, the findings and their counts.
pub(super) struct Ctx<'a> {
    pub(super) g: &'a Global,
    pub(super) a: &'a CheckArgs,
    pub(super) list: &'a [Finding],
    pub(super) errors: usize,
    pub(super) warnings: usize,
    pub(super) fails: usize,
    pub(super) root: &'a std::path::Path,
}

/// A CI format replaces the text: only the document goes to standard output, so it can be
/// redirected to a file, and the exit code still follows --fail-on.
pub(super) fn ci_output(cx: &Ctx, o: Output) -> Result<Output> {
    let Ctx { g, a, list, errors, warnings, fails, root } = *cx;
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
    // C4: the plain-text formats name the range too; sarif, gitlab and rdjson stay pure JSON.
    let out = match checked_range(root, a) {
        Some((_, line)) if matches!(a.format.as_str(), "github" | "editor") => out.head(line),
        _ => out,
    };
    Ok(if fails > 0 { out.code(Code::Findings) } else { out })
}
