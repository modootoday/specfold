//! A page body with each [[reference]] written as its target's link, or as plain text and
//! reported when it leads nowhere in the tree.

use super::page::Page;
use super::types::{Target, Unresolved};
use crate::paths::relative_from_file;
use std::collections::HashMap;

/// GitHub's heading slug: lower case, punctuation dropped, spaces to `-`.
pub fn github_slug(heading: &str) -> String {
    heading
        .trim()
        .to_lowercase()
        .chars()
        .filter_map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                Some(c)
            } else if c == ' ' {
                Some('-')
            } else {
                None
            }
        })
        .collect()
}

pub(super) fn rewrite(
    p: &Page,
    t: Target,
    pages: &[Page],
    by_idx: &HashMap<usize, usize>,
    unresolved: &mut Vec<Unresolved>,
    src: &str,
) -> String {
    let mut out = String::new();
    let mut at = 0;
    for l in &p.links {
        out.push_str(&p.body[at..l.span.0]);
        at = l.span.1;
        let dest = l.idx.and_then(|i| by_idx.get(&i)).map(|&i| &pages[i]);
        let text = l.label.clone().or_else(|| dest.map(|q| q.title.clone())).or_else(|| l.title.clone());
        let Some(q) = dest.filter(|_| l.reason.is_none()) else {
            let reason = l.reason.clone().unwrap_or_else(|| {
                if l.remote.is_some() {
                    format!("in remote {}", l.remote.clone().unwrap_or_default())
                } else {
                    "not part of this export".into()
                }
            });
            unresolved.push(Unresolved { path: src.into(), raw: l.raw.clone(), reason });
            let inner = l.raw.trim_start_matches("[[").trim_end_matches("]]");
            out.push_str(&text.unwrap_or_else(|| inner.split('|').next().unwrap_or(inner).trim().to_string()));
            continue;
        };
        let text = text.unwrap_or_else(|| q.title.clone());
        let heading = l.anchor.as_ref().map(|a| q.headings.get(a).cloned().unwrap_or_else(|| a.clone()));
        out.push_str(&match t {
            Target::Obsidian => {
                let h = heading.map(|h| format!("#{h}")).unwrap_or_default();
                format!("[[{}{h}|{text}]]", q.file.trim_end_matches(".md"))
            }
            Target::Mkdocs | Target::Docusaurus => {
                let a = l.anchor.as_ref().map(|a| format!("#{a}")).unwrap_or_default();
                format!("[{text}]({}{a})", relative_from_file(&p.file, &q.file))
            }
            Target::GithubWiki => {
                let a = heading.map(|h| format!("#{}", github_slug(&h))).unwrap_or_default();
                format!("[{text}]({}{a})", q.file.trim_end_matches(".md"))
            }
        });
    }
    out.push_str(&p.body[at..]);
    if t.keeps_anchors() { out } else { strip_anchors(&out) }
}

/// Removes `{#anchor}` from headings outside code fences.
pub(super) fn strip_anchors(text: &str) -> String {
    let re = regex::Regex::new(r"\s*\{#[A-Za-z0-9][A-Za-z0-9_.-]*\}\s*$").expect("static regex is valid");
    let mut fence = crate::markdown::Fence::default();
    let mut out = String::new();
    for line in text.split_inclusive('\n') {
        let tr = line.trim_start();
        fence.step(tr);
        if !fence.inside() && tr.starts_with('#') {
            let nl = if line.ends_with('\n') { "\n" } else { "" };
            out.push_str(&re.replace(line.trim_end_matches(['\n', '\r']), ""));
            out.push_str(nl);
        } else {
            out.push_str(line);
        }
    }
    out
}
