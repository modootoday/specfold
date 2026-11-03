//! Measures every submodule's visibility and works out the manifest to write: the order of the
//! file it replaces is kept, and nothing changes when the forge reports what the file holds.

use super::forge::{Forge, ForgeError};
use super::manifest::{self, Entry, Manifest};
use crate::workspace::projects::submodule_urls;
use std::path::Path;

/// The manifest a measurement produced, next to the one on disk.
#[derive(Debug, Clone)]
pub struct Measured {
    /// The manifest path, relative to the repository root.
    pub rel: String,
    /// Whether a manifest file exists at `rel` (readable or not).
    pub exists: bool,
    pub after: Manifest,
    /// The file text to write; the current text when nothing changed.
    pub text: String,
    pub changed: bool,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    /// Submodules whose URL or visibility changed.
    pub updated: Vec<String>,
}

/// Asks `forge` for every submodule and returns the manifest to write at `rel`, stamped `now`
/// only when something changed. No URL is sent anywhere until every one is known to the forge.
pub fn measure(root: &Path, rel: &str, forge: &dyn Forge, now: &str) -> Result<Measured, ForgeError> {
    let subs = submodule_urls(root);
    let mut repos = vec![];
    let mut unsupported = vec![];
    for (path, url) in &subs {
        let Some(url) = url else {
            unsupported.push(format!("{path} has no url in .gitmodules"));
            continue;
        };
        match forge.repository(url) {
            Ok(repo) => repos.push((path.clone(), url.clone(), repo)),
            Err(e) => unsupported.push(format!("{path}: {e}")),
        }
    }
    if !unsupported.is_empty() {
        return Err(ForgeError::Unsupported(unsupported.join("; ")));
    }
    let mut fresh: Vec<(String, Entry)> = vec![];
    for (path, url, repo) in repos {
        let visibility = forge.visibility(&repo).map_err(|e| match e {
            ForgeError::Failed(m) => ForgeError::Failed(format!("{path}: {m}")),
            other => other,
        })?;
        fresh.push((path, Entry { url, visibility }));
    }
    let current_text = std::fs::read_to_string(root.join(rel)).ok();
    let before = current_text.as_deref().and_then(manifest::parse);
    Ok(compare(rel, current_text, before, fresh, manifest::gitmodules_sha256(root), now))
}

/// The new manifest in the old one's order, with what was added, removed and updated.
fn compare(
    rel: &str,
    current_text: Option<String>,
    before: Option<Manifest>,
    fresh: Vec<(String, Entry)>,
    sha: String,
    now: &str,
) -> Measured {
    let old = before.as_ref().map(|b| b.submodules.clone()).unwrap_or_default();
    let has = |list: &[(String, Entry)], p: &str| list.iter().any(|(q, _)| q == p);
    let mut entries: Vec<(String, Entry)> =
        old.iter().filter_map(|(p, _)| fresh.iter().find(|(q, _)| q == p).cloned()).collect();
    let added: Vec<String> = fresh.iter().filter(|(p, _)| !has(&old, p)).map(|(p, _)| p.clone()).collect();
    entries.extend(fresh.iter().filter(|(p, _)| !has(&old, p)).cloned());
    let removed: Vec<String> = old.iter().filter(|(p, _)| !has(&fresh, p)).map(|(p, _)| p.clone()).collect();
    let updated: Vec<String> =
        old.iter().filter(|(p, e)| fresh.iter().any(|(q, f)| q == p && f != e)).map(|(p, _)| p.clone()).collect();
    let changed = match &before {
        Some(b) => b.gitmodules_sha256 != sha || b.submodules != entries,
        None => true,
    };
    let measured_at = match (&before, changed) {
        (Some(b), false) => b.measured_at.clone(),
        _ => now.to_string(),
    };
    let after = Manifest { gitmodules_sha256: sha, measured_at, submodules: entries };
    let exists = current_text.is_some();
    let text = match current_text {
        Some(t) if !changed => t,
        _ => manifest::render(&after),
    };
    Measured { rel: rel.to_string(), exists, after, text, changed, added, removed, updated }
}
