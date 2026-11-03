//! A unit's chapters and attachments moved into its bundle, with the `files` entries they get.

use super::names::name_of;
use super::types::ImportPlan;
use crate::corpus::Corpus;
use crate::files::hash;
use crate::import::sources::Unit;
use anyhow::Result;
use serde_json::{Value, json};

/// Plans the move of a unit's chapters and attachments into the bundle; returns the `files` entries.
pub(super) fn move_files(
    c: &Corpus,
    u: &Unit,
    base: &str,
    bundle_dir: &str,
    out: &mut ImportPlan,
) -> Result<Vec<Value>> {
    let mut files = vec![];
    // Chapters and attachments move: a sibling directory is not the unit's source, so removing
    // the source alone would leave them behind.
    for ch in &u.chapters {
        let rel = ch.strip_prefix(&format!("{base}/")).unwrap_or(name_of(ch));
        out.plan.copy(ch.clone(), format!("{bundle_dir}/{}", rel.replace('/', "-")));
        out.plan.remove(ch.clone());
    }
    for a in &u.attachments {
        let rel = a.strip_prefix(&format!("{base}/")).unwrap_or(name_of(a)).to_string();
        let (sha, bytes) = hash::file(&c.cfg.root.join(a))?;
        let store = if bytes > c.cfg.raw.files.inline_max_bytes { "store" } else { "repo" };
        if store == "store" {
            out.uploads.push((format!("{bundle_dir}/{rel}"), sha.clone()));
        }
        out.plan.copy(a.clone(), format!("{bundle_dir}/{rel}"));
        out.plan.remove(a.clone());
        files.push(json!({"path": rel, "sha256": sha, "bytes": bytes, "store": store}));
    }
    Ok(files)
}
