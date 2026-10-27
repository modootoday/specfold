//! Finding documents by words and filters.

use super::types::{Hit, Query};
use crate::corpus::Corpus;
use crate::model::{Doc, lifecycle};

fn passes_filters(d: &Doc, q: &Query) -> bool {
    let status = d.status().unwrap_or_default();
    (q.all || lifecycle::is_current(&status))
        && (q.types.is_empty() || q.types.contains(&d.id.ty))
        && (q.owners.is_empty() || q.owners.contains(&d.id.owner))
        && (q.statuses.is_empty() || q.statuses.contains(&status))
        && (q.tags.is_empty() || d.list_field("tags").iter().any(|t| q.tags.contains(t)))
}

fn score_meta(d: &Doc, words: &[String]) -> Option<u32> {
    let id = d.id.to_string().to_lowercase();
    let aliases: Vec<String> = d.list_field("aliases").iter().map(|a| a.to_lowercase()).collect();
    let hay = format!(
        "{id} {} {} {} {}",
        d.title.clone().unwrap_or_default().to_lowercase(),
        d.slug,
        aliases.join(" "),
        d.list_field("tags").join(" ").to_lowercase()
    );
    if words.is_empty() {
        return Some(1);
    }
    if !words.iter().all(|w| hay.contains(w.as_str())) {
        return None;
    }
    let joined = words.join(" ");
    Some(if id == joined || aliases.contains(&joined) { 100 } else { 10 + words.len() as u32 })
}

fn body_excerpt(c: &Corpus, d: &Doc, words: &[String]) -> Option<String> {
    for f in std::iter::once(&d.path).chain(&d.chapters) {
        let text = std::fs::read_to_string(c.cfg.root.join(f)).ok()?;
        for (n, line) in text.lines().enumerate() {
            let l = line.to_lowercase();
            if words.iter().all(|w| l.contains(w.as_str())) {
                return Some(format!("{f}:{}: {}", n + 1, line.trim().chars().take(140).collect::<String>()));
            }
        }
    }
    None
}

pub fn find(c: &Corpus, q: &Query) -> Vec<Hit> {
    let words: Vec<String> = q.words.iter().map(|w| w.to_lowercase()).collect();
    let mut hits: Vec<Hit> = c
        .docs
        .iter()
        .filter(|d| passes_filters(d, q))
        .filter_map(|d| {
            let (score, excerpt) = match score_meta(d, &words) {
                Some(s) => (s, None),
                None if q.text => (1, Some(body_excerpt(c, d, &words)?)),
                None => return None,
            };
            Some(Hit {
                id: d.id.to_string(),
                title: d.title.clone().unwrap_or_default(),
                status: d.status().unwrap_or_default(),
                path: d.path.clone(),
                score,
                excerpt,
            })
        })
        .collect();
    hits.sort_by(|a, b| b.score.cmp(&a.score).then(a.id.cmp(&b.id)));
    hits
}
