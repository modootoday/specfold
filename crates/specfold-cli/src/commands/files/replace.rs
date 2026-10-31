//! C202/C203: an attach whose bundle path already holds other bytes replaces that file. The
//! preview says so, the write needs --yes, a file spec never registered needs --force (as fetch),
//! and the old file goes to the trash first so the undo can put it back.

use crate::commands::write::preview::{TRASH, fresh_bin};
use anyhow::Result;
use specfold_core::files::{attach::Plan, hash};
use specfold_core::model::Doc;
use std::path::Path;

/// A planned file whose bundle path holds other content now.
pub struct Clash {
    /// Path from the repository root.
    pub path: String,
    /// Listed in the document's `files`, so specfold put it there.
    pub registered: bool,
}

/// The planned files that would overwrite other bytes. A document still to become a bundle has
/// no bundle folder yet, so nothing clashes.
pub fn clashes(root: &Path, doc: &Doc, plan: &Plan) -> Vec<Clash> {
    if plan.to_bundle.is_some() {
        return vec![];
    }
    let base = doc.base_dir();
    let listed: Vec<String> = doc.files().into_iter().map(|f| f.path).collect();
    plan.files
        .iter()
        .filter_map(|f| {
            let rel = format!("{base}/{}", f.path);
            let dst = root.join(&rel);
            let same = hash::file(&dst).map(|h| h.0 == f.sha256).unwrap_or(false);
            (dst.is_file() && !same).then(|| Clash { path: rel, registered: listed.contains(&f.path) })
        })
        .collect()
}

/// Copies the files about to be replaced into a fresh trash folder and gives the undo command.
pub fn keep(root: &Path, clashes: &[Clash]) -> Result<Option<String>> {
    if clashes.is_empty() {
        return Ok(None);
    }
    let bin = format!("{TRASH}/{}", fresh_bin(root)?);
    for c in clashes {
        let dst = root.join(&bin).join(&c.path);
        if let Some(dir) = dst.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::copy(root.join(&c.path), dst)?;
    }
    Ok(Some(format!("cp -Rp {}/. .", specfold_core::shell::quote(&bin))))
}
