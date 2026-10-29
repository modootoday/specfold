//! Translated documents: a translation names its source with `rel.translation_of` and records
//! the source it was made from as `source_rev`, a hash of the source body. No git history is
//! needed: the hash is compared with the source as it is now.

use crate::corpus::Corpus;
use crate::corpus::scan::hex;
use crate::graph::{Graph, Resolution};
use crate::markdown::frontmatter;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const RELATION: &str = "translation_of";
pub const KEY: &str = "source_rev";

/// `sha256:<12 hex>` of a document's body with the frontmatter removed and every run of
/// whitespace collapsed to one space, so line endings, wrapping and blank lines do not count.
pub fn rev_of_text(text: &str) -> String {
    let body = frontmatter::split(text).body;
    let norm = body.split_whitespace().collect::<Vec<_>>().join(" ");
    format!("sha256:{}", &hex(&Sha256::digest(norm.as_bytes()))[..12])
}

/// The current revision of a document, read from disk.
pub fn rev_of(c: &Corpus, idx: usize) -> Option<String> {
    let text = crate::corpus::overlay::read(&c.cfg.root.join(&c.docs[idx].path)).ok()?;
    Some(rev_of_text(&text))
}

/// What a document's `rel.translation_of` names, raw.
pub fn source_ref(c: &Corpus, idx: usize) -> Option<String> {
    c.docs[idx].rel().get(RELATION).and_then(|l| l.first().cloned())
}

/// The source document of a translation, when it resolves.
pub fn source_of(c: &Corpus, g: &Graph, idx: usize) -> Option<usize> {
    match g.resolve(&source_ref(c, idx)?) {
        Resolution::Doc { idx, .. } => Some(idx),
        _ => None,
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// source_rev matches the source.
    Current,
    /// The source changed after source_rev.
    Behind,
    /// No source_rev recorded.
    Unrecorded,
    /// rel.translation_of does not name a document.
    NoSource,
}

#[derive(Debug, Clone, Serialize)]
pub struct Translation {
    pub id: String,
    pub path: String,
    pub lang: Option<String>,
    pub source: String,
    pub state: State,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Report {
    pub translations: Vec<Translation>,
    /// Per language: sources with no translation in it.
    pub missing: BTreeMap<String, Vec<String>>,
    pub sources: usize,
}

/// The state of one translation.
pub fn state(c: &Corpus, g: &Graph, idx: usize) -> State {
    let Some(src) = source_of(c, g, idx) else { return State::NoSource };
    match c.docs[idx].str_field(KEY) {
        None => State::Unrecorded,
        Some(r) if Some(r.clone()) == rev_of(c, src) => State::Current,
        Some(_) => State::Behind,
    }
}

/// Every translation with its state, and the sources each language still lacks. `langs` limits
/// the coverage to those languages; empty means every language a translation declares.
pub fn report(c: &Corpus, g: &Graph, langs: &[String]) -> Report {
    let mut r = Report::default();
    let mut have: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut sources: Vec<usize> = vec![];
    for (i, d) in c.docs.iter().enumerate() {
        if source_ref(c, i).is_none() {
            continue;
        }
        let lang = d.str_field("lang");
        let src = source_of(c, g, i);
        if let (Some(l), Some(s)) = (&lang, src) {
            have.entry(l.clone()).or_default().push(s);
            if !sources.contains(&s) {
                sources.push(s);
            }
        }
        r.translations.push(Translation {
            id: d.id.to_string(),
            path: d.path.clone(),
            lang,
            source: src.map(|s| c.docs[s].id.to_string()).unwrap_or_else(|| source_ref(c, i).unwrap_or_default()),
            state: state(c, g, i),
        });
    }
    let wanted: Vec<String> = if langs.is_empty() { have.keys().cloned().collect() } else { langs.to_vec() };
    for l in wanted {
        let got = have.get(&l).cloned().unwrap_or_default();
        let miss: Vec<String> =
            sources.iter().filter(|s| !got.contains(s)).map(|&s| c.docs[s].id.to_string()).collect();
        r.missing.insert(l, miss);
    }
    r.sources = sources.len();
    r
}

#[cfg(test)]
mod tests {
    use super::rev_of_text;

    #[test]
    fn whitespace_and_frontmatter_do_not_count() {
        let a = rev_of_text("---\nid: x\n---\n# Guide\n\nOne line\nwrapped.\n");
        let b = rev_of_text("---\nid: x\nupdated: 2026\n---\r\n# Guide\r\n\r\n\r\nOne line wrapped.  \r\n");
        assert_eq!(a, b);
        assert!(a.starts_with("sha256:") && a.len() == 19);
        assert_ne!(a, rev_of_text("# Guide\n\nOne line changed.\n"));
    }
}
