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

pub fn fetch(c: &Corpus, doc: &Doc) -> Result<Vec<Fetched>> {
    let entries: Vec<_> = doc.files().into_iter().filter(|f| f.store == "store").collect();
    if entries.is_empty() {
        return Ok(vec![]);
    }
    let s = store::open(&c.cfg)?.context("no [files.store] is configured")?;
    let base = c.cfg.root.join(doc.base_dir());
    let mut out = vec![];
    for f in entries {
        let dst = base.join(&f.path);
        if dst.is_file() && hash::file(&dst).map(|h| h.0 == f.sha256).unwrap_or(false) {
            out.push(Fetched { path: f.path, state: "present".into() });
            continue;
        }
        s.get(&f.sha256, &dst)?;
        let (got, _) = hash::file(&dst)?;
        if got != f.sha256 {
            bail!("{} arrived with sha256 {got}, expected {}", f.path, f.sha256);
        }
        out.push(Fetched { path: f.path, state: "fetched".into() });
    }
    Ok(out)
}
