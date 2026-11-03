//! Finding the one document a reference names, and a document to show in examples.

use super::Session;
use anyhow::{Result, bail};
use specfold_core::graph::Resolution;

/// Resolves a reference to one document: id, alias, path, or a unique slug.
pub fn resolve(s: &Session, text: &str) -> Result<usize> {
    // C139: two files holding one id (I1) must not be picked silently; the path names one.
    let same: Vec<&str> = s.corpus.docs.iter().filter(|d| d.id.to_string() == text).map(|d| d.path.as_str()).collect();
    if same.len() > 1 {
        bail!("'{text}' is held by {} files: {}; name one by its path", same.len(), same.join(", "));
    }
    let rel = s.loc.repo_path(text);
    for candidate in [text.to_string(), rel] {
        match s.graph.resolve(&candidate) {
            Resolution::Doc { idx, .. } | Resolution::MissingAnchor { idx, .. } => return Ok(idx),
            Resolution::Ambiguous(list) => {
                let ids: Vec<String> = list.iter().map(|&i| s.corpus.docs[i].id.to_string()).collect();
                bail!("'{text}' is ambiguous: {}", ids.join(", "));
            }
            Resolution::Missing => {}
        }
    }
    let mut by_slug: Vec<usize> = (0..s.corpus.docs.len()).filter(|&i| s.corpus.docs[i].slug == text).collect();
    if by_slug.is_empty() {
        // The whole part after the owner (`20240401000000`) names the one id that ends with it.
        let tail = format!("/{text}");
        by_slug = (0..s.corpus.docs.len()).filter(|&i| s.corpus.docs[i].id.to_string().ends_with(&tail)).collect();
    }
    if by_slug.is_empty() {
        // `plan:root/payment-rollout`: the id with its slug in place of the timestamp.
        if let Some((head, slug)) = text.rsplit_once('/').filter(|(h, _)| h.contains(':')) {
            by_slug = (0..s.corpus.docs.len())
                .filter(|&i| {
                    let d = &s.corpus.docs[i];
                    let id = d.id.to_string();
                    d.slug == slug && id.rsplit_once('/').is_some_and(|(h, _)| h == head)
                })
                .collect();
        }
    }
    match by_slug.as_slice() {
        [i] => Ok(*i),
        [] => {
            // A partial id (a type, owner or timestamp prefix) gets the ids it could mean.
            let near: Vec<String> = s
                .corpus
                .docs
                .iter()
                .map(|d| d.id.to_string())
                .filter(|id| {
                    // A bare timestamp or slug part matches after the owner as well.
                    let local = id.rsplit('/').next().unwrap_or(id);
                    text.len() >= 3 && (id.starts_with(text) || local.starts_with(text))
                })
                .collect();
            match near.len() {
                0 if text.contains('/') && !text.contains(':') => bail!("'{text}' names no document"),
                0 if text.matches(':').count() >= 2
                    && text.split(':').next().is_some_and(|p| s.corpus.cfg.registry.get(p).is_none()) =>
                {
                    let p = text.split(':').next().unwrap_or_default();
                    let file = specfold_core::config::private_root::file().unwrap_or_default();
                    bail!(
                        "unknown id prefix '{p}': not in [remotes] nor [prefix] of {}; next: spec show {} --root <repo>",
                        file.display(),
                        &text[p.len() + 1..]
                    )
                }
                0 => {
                    // Words of a title or slug find it where an id prefix does not.
                    let words = if text.chars().all(|c| c.is_ascii_alphanumeric() || "-_:/.".contains(c)) {
                        text.to_string()
                    } else {
                        format!("'{}'", text.replace('\'', "'\\''"))
                    };
                    bail!("'{text}' names no document; next: spec find {words}")
                }
                1 => bail!("'{text}' names no document; 1 id starts with it: {0}; next: spec show {0}", near[0]),
                n => bail!(
                    "'{text}' names no document; {n} id(s) start with it: {}{}; next: spec show {}",
                    near.iter().take(5).cloned().collect::<Vec<_>>().join(", "),
                    if n > 5 { ", ..." } else { "" },
                    near[0]
                ),
            }
        }
        many => bail!(
            "'{text}' is ambiguous: {}",
            many.iter().map(|&i| s.corpus.docs[i].id.to_string()).collect::<Vec<_>>().join(", ")
        ),
    }
}

/// A document to put in an example command: of the preferred type if there is one, else the
/// most connected one, so the example shows something.
pub fn example_doc(s: &Session, prefer_ty: Option<&str>) -> Option<String> {
    let docs = &s.corpus.docs;
    if let Some(ty) = prefer_ty
        && let Some(d) = docs.iter().find(|d| d.id.ty == ty)
    {
        return Some(d.id.to_string());
    }
    let degree = |i: usize| s.graph.edges.iter().filter(|e| e.from == i || e.to == i).count();
    (0..docs.len()).max_by_key(|&i| (degree(i), std::cmp::Reverse(i))).map(|i| docs[i].id.to_string())
}
