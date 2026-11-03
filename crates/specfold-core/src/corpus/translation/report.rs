//! The state of each translation and the sources each language still lacks.

use super::layout::{is_translation, lang_of, source_of, source_ref};
use super::rev::{changes_since, parts_of, rev_of};
use super::types::{KEY, Report, State, Translation};
use crate::corpus::Corpus;
use crate::corpus::graph::Graph;
use std::collections::BTreeMap;

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
        // A translation of a translation (or of itself) is not a source; R1 reports it.
        if let (Some(l), Some(s)) = (&lang, src.filter(|&s| s != i && !is_translation(c, s))) {
            // C236: language tags match case-insensitively (pt-BR is pt-br), as `spec new` does.
            have.entry(l.to_ascii_lowercase()).or_default().push(s);
            if !sources.contains(&s) {
                sources.push(s);
            }
        }
        let st = state(c, g, i);
        let sections = match (&st, src, d.str_field(KEY)) {
            (State::Behind, Some(s), Some(rev)) => {
                let root = c.cfg.spec_home.clone();
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
        let got = have.get(&l.to_ascii_lowercase()).cloned().unwrap_or_default();
        let miss: Vec<String> =
            sources.iter().filter(|s| !got.contains(s)).map(|&s| c.docs[s].id.to_string()).collect();
        r.missing.insert(l, miss);
    }
    r.sources = sources.len();
    r
}
