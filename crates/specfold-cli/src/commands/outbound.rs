//! The guard gate for content leaving the repository: files uploaded to the store by
//! `spec attach` and documents handed out by `spec export`. Error-level hits refuse; warn-level
//! hits are shown and let through; a guard named with the command's override is reported but
//! does not refuse. Matched values are never printed, only the pattern or term mask.

use anyhow::{Result, bail};
use serde_json::{Value, json};
use specfold_core::check::finding::{Finding, Severity};
use specfold_core::config::{Config, GuardDef};
use specfold_core::guards;

pub struct Gate {
    /// (guard, finding, whether an override let it through)
    pub hits: Vec<(String, Finding, bool)>,
}

impl Gate {
    pub fn refuses(&self) -> bool {
        self.hits.iter().any(|(_, f, allowed)| !allowed && f.severity == Severity::Error)
    }

    pub fn json(&self) -> Value {
        Value::Array(
            self.hits
                .iter()
                .map(|(g, f, allowed)| {
                    json!({"guard": g, "path": f.path, "line": f.line, "level": level(f), "message": f.message, "allowed": allowed})
                })
                .collect(),
        )
    }

    pub fn lines(&self) -> Vec<String> {
        self.hits
            .iter()
            .map(|(_, f, allowed)| {
                let tag = if *allowed { "allowed" } else { level(f) };
                format!("{tag}: {}:{} {}", f.path, f.line, f.message)
            })
            .collect()
    }

    /// Adds the built-in secrets check over `blobs`, skipping lines a guard already reported.
    /// `warn_only` keeps the hits from refusing (files that stay in the repository).
    pub fn add_builtin(&mut self, blobs: &[(String, Vec<u8>)], allow: &[String], warn_only: bool) {
        for (g, mut f) in guards::builtin_secrets(blobs) {
            if self.hits.iter().any(|(_, h, _)| h.path == f.path && h.line == f.line) {
                continue;
            }
            if warn_only {
                f.severity = Severity::Warn;
            }
            let allowed = allow.contains(&g);
            self.hits.push((g, f, allowed));
        }
    }

    /// The guards whose unallowed error hits refuse, for the override hint.
    pub fn refusing(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .hits
            .iter()
            .filter(|(_, f, a)| !a && f.severity == Severity::Error)
            .map(|(g, _, _)| g.clone())
            .collect();
        v.dedup();
        v
    }
}

fn level(f: &Finding) -> &'static str {
    if f.severity == Severity::Error { "error" } else { "warn" }
}

/// Runs the guards `pick` selects over `blobs`; hits of the guards in `allow` do not refuse.
pub fn gate(
    cfg: &Config,
    pick: impl Fn(&GuardDef) -> bool,
    blobs: &[(String, Vec<u8>)],
    allow: &[String],
) -> Result<Gate> {
    for a in allow {
        // `secrets` also names the built-in check, which needs no configured guard.
        if a != "secrets" && !cfg.raw.guards.iter().any(|g| &g.name == a) {
            let mut have: Vec<&str> = cfg.raw.guards.iter().map(|g| g.name.as_str()).collect();
            have.push("secrets");
            bail!("no guard named '{a}' (have: {})", have.join(", "));
        }
    }
    let hits = guards::scan_blobs(cfg, pick, blobs)?
        .into_iter()
        .map(|(g, f)| {
            let allowed = allow.contains(&g);
            (g, f, allowed)
        })
        .collect();
    Ok(Gate { hits })
}

/// Refuses text holding a redaction marker the document does not already hold: it was read
/// through `spec mcp`, and writing it back would replace the real value with the marker.
pub fn refuse_redacted(root: &std::path::Path, doc_path: &str, text: &str) -> Result<()> {
    let marker = specfold_core::guards::secrets::MARKER;
    if !text.contains(marker) {
        return Ok(());
    }
    let current = std::fs::read_to_string(root.join(doc_path)).unwrap_or_default();
    if current.contains(marker) {
        return Ok(());
    }
    bail!(
        "the text holds '{marker}…]', which spec mcp puts in place of a secret; writing it would replace the real value. Edit that line in the file itself, or leave it out of the change; to add new text without resending the redacted line, use spec edit <id> --append (or --section <name>); next: spec help edit"
    )
}
