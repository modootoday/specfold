//! Which document translates which: `rel.translation_of`, or the pair the `[translations]` layout
//! infers from where a document sits, and the language each one is in.

use super::types::{Inferred, RELATION};
use crate::corpus::Corpus;
use crate::graph::{Graph, Resolution};

/// Why `lang` is not a language tag (BCP 47 shape: a 2-3 letter language, then subtags of 1-8
/// letters or digits, `ko`, `pt-BR`, `zh-Hant-TW`), or `None` when it is one.
pub fn lang_problem(lang: &str) -> Option<String> {
    let mut parts = lang.split('-');
    let first = parts.next().unwrap_or("");
    let ok = (2..=3).contains(&first.len())
        && first.chars().all(|c| c.is_ascii_alphabetic())
        && parts.all(|p| (1..=8).contains(&p.len()) && p.chars().all(|c| c.is_ascii_alphanumeric()));
    (!ok).then(|| format!("lang '{lang}' is not a language tag (BCP 47) like ko, ja or pt-BR"))
}

/// What a document's `rel.translation_of` names, raw.
pub fn source_ref(c: &Corpus, idx: usize) -> Option<String> {
    c.docs[idx].rel().get(RELATION).and_then(|l| l.first().cloned())
}

/// The source document of a translation, when it resolves: `rel.translation_of` when set,
/// otherwise the source the `[translations]` layout pairs it with.
pub fn source_of(c: &Corpus, g: &Graph, idx: usize) -> Option<usize> {
    match source_ref(c, idx) {
        Some(r) => match g.resolve(&r) {
            Resolution::Doc { idx, .. } => Some(idx),
            _ => None,
        },
        None => inferred(c, idx).and_then(|p| p.source),
    }
}

/// Whether a document is a translation: it names a source, or the layout places it as one.
pub fn is_translation(c: &Corpus, idx: usize) -> bool {
    source_ref(c, idx).is_some() || position(c, idx).is_some()
}

/// The language of a document: frontmatter `lang`, else the layout's language marker, else
/// `[translations] source_lang` for a document that is not a translation.
pub fn lang_of(c: &Corpus, idx: usize) -> Option<String> {
    c.docs[idx]
        .str_field("lang")
        .or_else(|| position(c, idx).map(|p| p.0))
        .or_else(|| (!is_translation(c, idx)).then(|| c.cfg.raw.translations.source_lang.clone()).flatten())
}

/// Where the layout places a document as a translation: its language and the (type, spec root,
/// slug) its source has.
fn position(c: &Corpus, idx: usize) -> Option<(String, (String, String, String))> {
    use crate::config::types::TranslationLayout as L;
    let t = &c.cfg.raw.translations;
    let d = &c.docs[idx];
    let root = spec_dir(c, idx);
    match t.layout {
        L::None => None,
        L::Suffix => t.languages.iter().find_map(|l| {
            let base = d.slug.strip_suffix(&format!("-{}", l.to_ascii_lowercase()))?;
            (!base.is_empty()).then(|| (l.clone(), (d.id.ty.clone(), root.clone(), base.to_string())))
        }),
        L::Dir => {
            let segs: Vec<&str> = root.split('/').collect();
            let at = segs.iter().rposition(|s| t.languages.iter().any(|l| l == s))?;
            let lang = segs[at].to_string();
            let rest: Vec<&str> = segs.iter().enumerate().filter(|(i, _)| *i != at).map(|(_, s)| *s).collect();
            Some((lang, (d.id.ty.clone(), rest.join("/"), d.slug.clone())))
        }
    }
}

/// The pair the `[translations]` layout infers for a document, when it is in a translation
/// position.
pub fn inferred(c: &Corpus, idx: usize) -> Option<Inferred> {
    let (lang, want) = position(c, idx)?;
    let candidates: Vec<usize> = (0..c.docs.len())
        .filter(|&j| j != idx && position(c, j).is_none())
        .filter(|&j| {
            let d = &c.docs[j];
            d.id.ty == want.0 && d.slug == want.2 && spec_dir(c, j) == want.1
        })
        .collect();
    let source = (candidates.len() == 1).then(|| candidates[0]);
    Some(Inferred { lang, source, candidates })
}

/// The spec directory that holds a document (`docs/.spec`), from the corpus roots.
fn spec_dir(c: &Corpus, idx: usize) -> String {
    c.spec_root_of(idx).map(|r| r.dir.clone()).unwrap_or_default()
}
