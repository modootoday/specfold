//! Which governing documents a version-only manifest change leaves alone, read once per path and
//! once per document.

use super::diff::{is_manifest, version_bump};
use crate::check::context::Ctx;
use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};

/// D1's memory of the manifests in a run and the documents it has read.
pub(in crate::check::rules::lifecycle) struct Quiet<'a, 'c> {
    ctx: &'a Ctx<'c>,
    bumps: RefCell<HashMap<String, Option<BTreeSet<String>>>>,
    texts: RefCell<HashMap<usize, String>>,
}

impl<'a, 'c> Quiet<'a, 'c> {
    /// Nothing read yet.
    pub(in crate::check::rules::lifecycle) fn new(ctx: &'a Ctx<'c>) -> Self {
        Quiet { ctx, bumps: RefCell::default(), texts: RefCell::default() }
    }

    /// Whether document `doc` is not asked about `path`: the change of `path` only moves package
    /// versions and the document names none of those packages.
    pub(in crate::check::rules::lifecycle) fn skips(&self, path: &str, doc: usize) -> bool {
        if !is_manifest(path) {
            return false;
        }
        let mut bumps = self.bumps.borrow_mut();
        let moved = bumps.entry(path.to_string()).or_insert_with(|| {
            let (old, new) = (self.ctx.previous(path)?, self.ctx.current(path)?);
            version_bump(path, &old, &new)
        });
        let Some(moved) = moved else { return false };
        let mut texts = self.texts.borrow_mut();
        let text = texts.entry(doc).or_insert_with(|| self.text(doc));
        !moved.iter().any(|p| names(text, p))
    }

    /// The document's own files as checked: its main file and its chapters.
    fn text(&self, doc: usize) -> String {
        let d = &self.ctx.corpus.docs[doc];
        std::iter::once(&d.path).chain(&d.chapters).filter_map(|p| self.ctx.current(p)).collect::<Vec<_>>().join("\n")
    }
}

/// Whether `text` names `package` as a whole name, not as part of a longer one
/// (`@acme/tool` is not named by `@acme/tool-linux-x64`).
fn names(text: &str, package: &str) -> bool {
    let part = |c: char| c.is_alphanumeric() || matches!(c, '-' | '_' | '/' | '@');
    text.match_indices(package).any(|(at, _)| {
        let before = text[..at].chars().next_back().is_none_or(|c| !part(c) && c != '.');
        let after = text[at + package.len()..].chars().next().is_none_or(|c| !part(c));
        before && after
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_package_is_named_only_whole() {
        assert!(super::names("uses `@acme/tool` at 1.0.", "@acme/tool"));
        assert!(!super::names("uses @acme/tool-linux-x64", "@acme/tool"));
        assert!(!super::names("uses @acme/toolbox", "@acme/tool"));
    }
}
