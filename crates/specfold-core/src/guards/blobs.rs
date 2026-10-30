//! Guards over blobs about to leave the repository: export and upload checks.

use super::{compile, prove, scan, secrets};
use crate::check::finding::Finding;
use crate::config::{Config, GuardDef};
use anyhow::Result;

/// Runs the guards `pick` selects over (label, bytes) about to leave the repository, each with its
/// own settings whatever its paths; deny_files and path patterns read the label. Baselines do not
/// apply, a known hit is still leaving. Each finding names the guard that raised it.
pub fn scan_blobs(
    cfg: &Config,
    pick: impl Fn(&GuardDef) -> bool,
    blobs: &[(String, Vec<u8>)],
) -> Result<Vec<(String, Finding)>> {
    let mut out = vec![];
    for g in cfg.raw.guards.iter().filter(|g| pick(g)) {
        let c = compile::compile(cfg, g)?;
        prove::prove(g, &c)?;
        for (label, bytes) in blobs {
            out.extend(scan::scan_blob(g, &c, label, bytes.clone()).into_iter().map(|f| (g.name.clone(), f)));
        }
    }
    Ok(out)
}

/// The built-in secret patterns over `blobs`, with no guard configured: the check `spec export`
/// always runs. Hits are errors named `secrets`; matched values are never in the message.
pub fn builtin_secrets(blobs: &[(String, Vec<u8>)]) -> Vec<(String, Finding)> {
    let mut out = vec![];
    for (label, bytes) in blobs {
        let text = String::from_utf8_lossy(bytes);
        for h in secrets::find(&text) {
            let line = text[..h.start].matches('\n').count() + 1;
            let msg = format!("[secrets] {} (built-in check)", h.name);
            out.push(("secrets".to_string(), Finding::error("U1", label.clone(), line, msg)));
        }
    }
    out
}

/// Guards that read content for secrets or denied text, the ones an upload must pass.
pub fn reads_content(g: &GuardDef) -> bool {
    g.secrets
        || !g.deny_terms.is_empty()
        || g.deny_terms_file.is_some()
        || g.deny_terms_env.is_some()
        || !g.deny_regex.is_empty()
        || !g.deny_path_regex.is_empty()
        || !g.deny_files.is_empty()
}
