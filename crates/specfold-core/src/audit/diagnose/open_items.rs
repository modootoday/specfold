//! Open items: configured markers of unfinished work left in current documents.

use super::super::types::{Item, Kind};
use crate::corpus::Corpus;
use crate::model::lifecycle;
use regex::{Regex, RegexBuilder};

fn matcher(patterns: &[String]) -> Option<Regex> {
    if patterns.is_empty() {
        return None;
    }
    let alt: Vec<String> = patterns.iter().map(|p| regex::escape(p)).collect();
    RegexBuilder::new(&alt.join("|")).build().ok()
}

pub fn run(c: &Corpus, docs: &[usize], out: &mut Vec<Item>) {
    let Some(re) = matcher(&c.cfg.raw.audit.open_item_patterns) else { return };
    for &i in docs {
        let d = &c.docs[i];
        if !d.status().as_deref().is_some_and(lifecycle::is_current) {
            continue;
        }
        for file in std::iter::once(&d.path).chain(&d.chapters) {
            let Ok(text) = std::fs::read_to_string(c.cfg.root.join(file)) else { continue };
            for (n, line) in text.lines().enumerate() {
                if let Some(m) = re.find(line) {
                    let excerpt: String = line.trim().chars().take(120).collect();
                    out.push(Item {
                        kind: Kind::OpenItem,
                        subject: d.id.to_string(),
                        path: file.clone(),
                        line: n + 1,
                        detail: format!("'{}': {excerpt}", m.as_str()),
                        next: None,
                    });
                }
            }
        }
    }
}
