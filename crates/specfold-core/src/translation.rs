//! Translated documents: a translation names its source with `rel.translation_of` and records
//! the source it was made from as `source_rev`, a hash of the source body (the main file and the
//! bundle's chapters). No git history is needed to see that a translation is behind: the hash is
//! compared with the source as it is now. Where git has the recorded revision, the sections and
//! paragraphs that changed since are named as well.

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

/// The revision of a whole source: its main file, then its chapters in path order. A document
/// without chapters has the revision of its main file.
pub fn rev_of_parts(parts: &[Part]) -> String {
    let norm = parts.iter().flat_map(|p| p.body.split_whitespace()).collect::<Vec<_>>().join(" ");
    format!("sha256:{}", &hex(&Sha256::digest(norm.as_bytes()))[..12])
}

/// One file of a source: `label` is empty for the main file and the chapter path otherwise.
#[derive(Debug, Clone)]
pub struct Part {
    pub label: String,
    pub body: String,
}

/// The files of a document read with `read` (the working tree, the index or a commit), main
/// file first; None when the main file cannot be read. Unreadable chapters are left out.
pub fn parts_of(c: &Corpus, idx: usize, read: impl Fn(&str) -> Option<String>) -> Option<Vec<Part>> {
    let d = &c.docs[idx];
    let main = read(&d.path)?;
    let mut parts = vec![Part { label: String::new(), body: frontmatter::split(&main).body.to_string() }];
    let mut chapters = d.chapters.clone();
    chapters.sort();
    for ch in chapters {
        if let Some(t) = read(&ch) {
            let label = ch.rsplit('/').next().unwrap_or(&ch).to_string();
            parts.push(Part { label, body: frontmatter::split(&t).body.to_string() });
        }
    }
    Some(parts)
}

/// The current revision of a document, read from disk.
pub fn rev_of(c: &Corpus, idx: usize) -> Option<String> {
    let root = c.cfg.root.clone();
    let parts = parts_of(c, idx, |p| crate::corpus::overlay::read(&root.join(p)).ok())?;
    Some(rev_of_parts(&parts))
}

/// How one section of a source differs from the revision a translation was made from.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SectionChange {
    /// The heading text, prefixed with the chapter file for chapters; `(top)` before the first
    /// heading.
    pub section: String,
    /// `changed`, `added` or `removed`.
    pub change: String,
    /// Paragraphs that differ (for `changed`) or that the section has (otherwise).
    pub paragraphs: usize,
}

/// Sections of one body: heading text and normalized paragraphs, fenced code kept whole.
fn sections(label: &str, body: &str) -> Vec<(String, Vec<String>)> {
    let name = |h: &str| if label.is_empty() { h.to_string() } else { format!("{label}: {h}") };
    let mut out: Vec<(String, Vec<String>)> = vec![(name("(top)"), vec![])];
    let mut para: Vec<&str> = vec![];
    let mut fence = false;
    let flush = |para: &mut Vec<&str>, out: &mut Vec<(String, Vec<String>)>| {
        let t = para.iter().flat_map(|l| l.split_whitespace()).collect::<Vec<_>>().join(" ");
        if !t.is_empty() {
            out.last_mut().expect("one section").1.push(t);
        }
        para.clear();
    };
    for line in body.lines() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            fence = !fence;
            para.push(line);
            continue;
        }
        if !fence && t.starts_with('#') && t.trim_start_matches('#').starts_with([' ', '\t']) {
            flush(&mut para, &mut out);
            out.push((name(t.trim_start_matches('#').trim()), vec![]));
        } else if !fence && t.is_empty() {
            flush(&mut para, &mut out);
        } else {
            para.push(line);
        }
    }
    flush(&mut para, &mut out);
    out.retain(|(h, ps)| !(ps.is_empty() && h.ends_with("(top)")));
    out
}

/// The sections that differ between two versions of a source, in the order of the new one
/// (removed sections last).
pub fn section_changes(old: &[Part], new: &[Part]) -> Vec<SectionChange> {
    let all = |parts: &[Part]| parts.iter().flat_map(|p| sections(&p.label, &p.body)).collect::<Vec<_>>();
    let (old, new) = (all(old), all(new));
    let mut out = vec![];
    for (h, ps) in &new {
        match old.iter().find(|(o, _)| o == h) {
            None => out.push(SectionChange { section: h.clone(), change: "added".into(), paragraphs: ps.len() }),
            Some((_, was)) => {
                let gone = was.iter().filter(|p| !ps.contains(p)).count();
                let came = ps.iter().filter(|p| !was.contains(p)).count();
                let n = gone.max(came);
                if n > 0 || was.len() != ps.len() {
                    out.push(SectionChange { section: h.clone(), change: "changed".into(), paragraphs: n.max(1) });
                }
            }
        }
    }
    for (h, ps) in &old {
        if !new.iter().any(|(n, _)| n == h) {
            out.push(SectionChange { section: h.clone(), change: "removed".into(), paragraphs: ps.len() });
        }
    }
    out
}

/// The source as it was at `rev`, found in git history (the commits that touched its files,
/// newest first, at most 200); None without git or when no commit has that revision.
pub fn parts_at_rev(c: &Corpus, idx: usize, rev: &str) -> Option<Vec<Part>> {
    let root = &c.cfg.root;
    let d = &c.docs[idx];
    let mut commits: Vec<String> = vec![];
    for f in std::iter::once(&d.path).chain(&d.chapters) {
        for r in crate::workspace::vcs::history(root, f).unwrap_or_default().into_iter().rev() {
            if !commits.contains(&r.hash) {
                commits.push(r.hash);
            }
        }
    }
    commits.into_iter().take(200).find_map(|h| {
        let parts = parts_of(c, idx, |p| crate::workspace::vcs::show(root, &h, p))?;
        (rev_of_parts(&parts) == rev).then_some(parts)
    })
}

/// What changed in a translation's source since `rev`, when git has that revision.
pub fn changes_since(c: &Corpus, src: usize, rev: &str, now: &[Part]) -> Option<Vec<SectionChange>> {
    Some(section_changes(&parts_at_rev(c, src, rev)?, now))
}

/// `Decision (2 paragraphs changed), Risks (new)`: for messages.
pub fn describe(changes: &[SectionChange]) -> String {
    changes
        .iter()
        .map(|ch| match ch.change.as_str() {
            "added" => format!("{} (new)", ch.section),
            "removed" => format!("{} (removed)", ch.section),
            _ => format!(
                "{} ({} paragraph{} changed)",
                ch.section,
                ch.paragraphs,
                if ch.paragraphs == 1 { "" } else { "s" }
            ),
        })
        .collect::<Vec<_>>()
        .join(", ")
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

/// The directory that holds a document's type directories (`docs/.spec`), from its path.
fn spec_root(c: &Corpus, idx: usize) -> String {
    let d = &c.docs[idx];
    let marker = format!("/{}/", d.id.ty);
    let p = format!("/{}", d.dir.as_deref().unwrap_or(&d.path));
    match p.rfind(&marker) {
        Some(i) => p[1..i.max(1)].to_string(),
        None => String::new(),
    }
}

/// Where the layout places a document as a translation: its language and the (type, spec root,
/// slug) its source has.
fn position(c: &Corpus, idx: usize) -> Option<(String, (String, String, String))> {
    use crate::config::types::TranslationLayout as L;
    let t = &c.cfg.raw.translations;
    let d = &c.docs[idx];
    let root = spec_root(c, idx);
    match t.layout {
        L::None => None,
        L::Suffix => t.languages.iter().find_map(|l| {
            let base = d.slug.strip_suffix(&format!("-{l}"))?;
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

/// A pair the layout infers for a document in a translation position.
#[derive(Debug, Clone)]
pub struct Inferred {
    pub lang: String,
    /// The source, when exactly one document is in the source position.
    pub source: Option<usize>,
    /// Every document in the source position (more than one is ambiguous).
    pub candidates: Vec<usize>,
}

/// The pair the `[translations]` layout infers for a document, when it is in a translation
/// position.
pub fn inferred(c: &Corpus, idx: usize) -> Option<Inferred> {
    let (lang, want) = position(c, idx)?;
    let candidates: Vec<usize> = (0..c.docs.len())
        .filter(|&j| j != idx && position(c, j).is_none())
        .filter(|&j| {
            let d = &c.docs[j];
            d.id.ty == want.0 && d.slug == want.2 && spec_root(c, j) == want.1
        })
        .collect();
    let source = (candidates.len() == 1).then(|| candidates[0]);
    Some(Inferred { lang, source, candidates })
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
    /// For a translation behind its source: the sections changed since source_rev, when git has
    /// that revision.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<SectionChange>,
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
        if !is_translation(c, i) {
            continue;
        }
        let lang = lang_of(c, i);
        let src = source_of(c, g, i);
        if let (Some(l), Some(s)) = (&lang, src) {
            have.entry(l.clone()).or_default().push(s);
            if !sources.contains(&s) {
                sources.push(s);
            }
        }
        let st = state(c, g, i);
        let sections = match (&st, src, d.str_field(KEY)) {
            (State::Behind, Some(s), Some(rev)) => {
                let root = c.cfg.root.clone();
                parts_of(c, s, |p| crate::corpus::overlay::read(&root.join(p)).ok())
                    .and_then(|now| changes_since(c, s, &rev, &now))
                    .unwrap_or_default()
            }
            _ => vec![],
        };
        r.translations.push(Translation {
            id: d.id.to_string(),
            path: d.path.clone(),
            lang,
            source: src
                .map(|s| c.docs[s].id.to_string())
                .unwrap_or_else(|| source_ref(c, i).unwrap_or_else(|| "(none)".into())),
            state: st,
            sections,
        });
    }
    let configured = &c.cfg.raw.translations.languages;
    let wanted: Vec<String> = if !langs.is_empty() {
        langs.to_vec()
    } else if !configured.is_empty() {
        configured.clone()
    } else {
        have.keys().cloned().collect()
    };
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
    use super::{Part, rev_of_parts, rev_of_text, section_changes};

    fn one(body: &str) -> Vec<Part> {
        vec![Part { label: String::new(), body: body.into() }]
    }

    #[test]
    fn sections_and_paragraphs_that_changed_are_named() {
        let old = one("Intro.\n\n## Context\n\nA.\n\nB.\n\n## Decision\n\nUse X.\n\n## Old\n\nGone.\n");
        let new = one("Intro.\n\n## Context\n\nA.\n\nB, longer.\n\n## Decision\n\nUse   X.\n\n## Risks\n\nR.\n");
        let got: Vec<(String, String, usize)> =
            section_changes(&old, &new).into_iter().map(|c| (c.section, c.change, c.paragraphs)).collect();
        assert_eq!(
            got,
            vec![
                ("Context".into(), "changed".into(), 1),
                ("Risks".into(), "added".into(), 1),
                ("Old".into(), "removed".into(), 1)
            ]
        );
    }

    #[test]
    fn a_document_without_chapters_keeps_the_revision_of_its_body() {
        let text = "---\nid: x\n---\n# Guide\n\nOne.\n";
        assert_eq!(rev_of_parts(&one("# Guide\n\nOne.\n")), rev_of_text(text));
        let with = vec![one("# Guide\n\nOne.\n").remove(0), Part { label: "a.md".into(), body: "More.".into() }];
        assert_ne!(rev_of_parts(&with), rev_of_text(text));
    }

    #[test]
    fn whitespace_and_frontmatter_do_not_count() {
        let a = rev_of_text("---\nid: x\n---\n# Guide\n\nOne line\nwrapped.\n");
        let b = rev_of_text("---\nid: x\nupdated: 2026\n---\r\n# Guide\r\n\r\n\r\nOne line wrapped.  \r\n");
        assert_eq!(a, b);
        assert!(a.starts_with("sha256:") && a.len() == 19);
        assert_ne!(a, rev_of_text("# Guide\n\nOne line changed.\n"));
    }
}
