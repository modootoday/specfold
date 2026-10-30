//! Bringing a document's stored attachments back onto disk, verified by hash.

use super::{hash, store};
use crate::corpus::Corpus;
use crate::model::Doc;
use anyhow::{Context, Result, bail};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Fetched {
    pub path: String,
    /// `fetched`, `present` (already on disk with the right hash).
    pub state: String,
}

/// Brings a document's stored files to disk. A file already there with other content is left
/// alone unless `force` (CLI-4): nothing is fetched while any would be overwritten.
pub fn fetch(c: &Corpus, doc: &Doc, force: bool) -> Result<Vec<Fetched>> {
    let entries: Vec<_> = doc.files().into_iter().filter(|f| f.store == "store").collect();
    if entries.is_empty() {
        return Ok(vec![]);
    }
    let s = store::open(&c.cfg)?.context("no [files.store] is configured")?;
    let base = c.cfg.root.join(doc.base_dir());
    if !force {
        let other: Vec<String> = entries
            .iter()
            .filter(|f| {
                let dst = base.join(&f.path);
                dst.is_file() && !hash::file(&dst).map(|h| h.0 == f.sha256).unwrap_or(false)
            })
            .map(|f| f.path.clone())
            .collect();
        if !other.is_empty() {
            bail!(
                "{} already exist(s) with other content; nothing was fetched. Move it aside, or run spec fetch {} --force to overwrite",
                other.join(", "),
                doc.id
            );
        }
    }
    let mut out = vec![];
    for f in entries {
        let dst = base.join(&f.path);
        if dst.is_file() && hash::file(&dst).map(|h| h.0 == f.sha256).unwrap_or(false) {
            out.push(Fetched { path: f.path, state: "present".into() });
            continue;
        }
        // C127: the bytes land beside the target and replace it only once verified.
        let name = dst.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let part = dst.with_file_name(format!(".{name}.specfold-part"));
        if let Err(e) = s.get(&f.sha256, &part) {
            let _ = std::fs::remove_file(&part);
            return Err(e);
        }
        let (got, _) = hash::file(&part)?;
        if got != f.sha256 {
            let _ = std::fs::remove_file(&part);
            // An environment problem (the store holds other bytes), not a usage error: exit 3.
            return Err(anyhow::Error::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("{} arrived with sha256 {got}, expected {}; nothing was written", f.path, f.sha256),
            )));
        }
        std::fs::rename(&part, &dst)?;
        out.push(Fetched { path: f.path, state: "fetched".into() });
    }
    Ok(out)
}
