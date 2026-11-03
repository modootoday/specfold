//! What an export runs over the files about to leave: the built-in secrets check and the public
//! guards, over the exported text and the source files it comes from.

use crate::cli::args::setup::ExportArgs;
use crate::commands::outbound;
use crate::output::{Code, Output};
use anyhow::Result;

/// Runs the built-in secrets check and the public guards over what is about to leave; the hits
/// that let it through are added to `o` either way.
pub(super) fn guard(
    cfg: &specfold_core::Config,
    a: &ExportArgs,
    blobs: &[(String, Vec<u8>)],
    sources: &[(String, Vec<u8>)],
    o: Output,
) -> Result<(Output, bool)> {
    if a.no_guard {
        return Ok((o.foot("secrets check and public guards skipped (--no-guard)"), false));
    }
    let mut gate = outbound::gate(cfg, |g| g.public, blobs, &[])?;
    // The built-in secrets check runs unless a public guard already reads secrets, over the
    // source files so a hit names the line to fix; a line already reported is not repeated.
    let covered = cfg.raw.guards.iter().any(|g| g.public && g.secrets);
    let builtin = if covered { vec![] } else { specfold_core::guards::builtin_secrets(sources) };
    for (g, f) in builtin {
        if !gate.hits.iter().any(|(_, h, _)| h.path == f.path && h.line == f.line) {
            gate.hits.push((g, f, false));
        }
    }
    let mut o = o.field("guard", gate.json());
    for l in gate.lines() {
        o = o.head(l);
    }
    if gate.refuses() {
        // `spec guard` re-runs a configured guard; `spec guard secrets` the built-in check.
        let refusing_guard = gate.refusing().iter().any(|g| g != "secrets");
        let n = gate.hits.iter().filter(|(_, f, _)| f.severity == specfold_core::check::Severity::Error).count();
        return Ok((
            o.head(format!(
                "nothing exported: {} found {n} hit(s); remove the value, or pass --no-guard to skip the check",
                gate.refusing().join(", ")
            ))
            .code(Code::Findings)
            .next(if refusing_guard { "spec guard --range all" } else { "spec guard secrets" }),
            true,
        ));
    }
    Ok((o, false))
}

/// One line naming the exported documents' attachments the secrets check could not read.
pub(super) fn unscanned(c: &specfold_core::corpus::Corpus, only: &[usize]) -> Option<String> {
    let max = c.cfg.raw.files.scan_max_bytes;
    let mut skipped: Vec<String> = only
        .iter()
        .flat_map(|&i| specfold_core::guards::secrets::attachment_scan(&c.cfg.root, &c.docs[i], max).1)
        .collect();
    skipped.sort();
    skipped.dedup();
    specfold_core::guards::secrets::skipped_note(&c.cfg.root, &skipped, max)
}

/// The files the exported documents come from (main file and chapters) and their repository
/// text attachments up to [files] scan_max_bytes (the set rule U2 reads), as guard blobs.
pub(super) fn source_blobs(c: &specfold_core::corpus::Corpus, only: &[usize]) -> Vec<(String, Vec<u8>)> {
    let mut blobs = vec![];
    let max = c.cfg.raw.files.scan_max_bytes;
    for &i in only {
        let d = &c.docs[i];
        let attachments = specfold_core::guards::secrets::scanned_attachments(&c.cfg.root, d, max);
        for path in std::iter::once(&d.path).chain(&d.chapters).chain(&attachments) {
            if let Ok(bytes) = std::fs::read(c.cfg.root.join(path)) {
                blobs.push((path.clone(), bytes));
            }
        }
    }
    blobs
}
