//! Reads the parts of a Markdown body specfold cares about: `[[references]]`, heading anchors,
//! and paths that cite spec files directly. Fenced code blocks and inline code are skipped, so
//! examples in documentation never count as references.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    pub text: String,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Anchor {
    pub name: String,
    pub heading: String,
    pub line: usize,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Body {
    pub links: Vec<Link>,
    pub anchors: Vec<Anchor>,
    /// Paths into a spec directory written as plain text, e.g. `services/x/.spec/plan/…md`.
    pub path_cites: Vec<Link>,
    pub first_heading: Option<String>,
    pub lines: usize,
}

/// `[[target]]` and `[[target|label]]`; group 1 is the target.
pub fn wiki_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\[\[([^\[\]\n|]+)(?:\|[^\[\]\n]*)?\]\]").unwrap())
}

fn anchor_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\{#([A-Za-z0-9][A-Za-z0-9_.-]*)\}\s*$").unwrap())
}

fn inline_code_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"`+[^`]*`+").unwrap())
}

/// Builds the regex that finds plain-text paths into `<spec_dir>/` ending in `.md`.
pub fn path_cite_re(spec_dir: &str) -> Regex {
    let dir = regex::escape(spec_dir);
    Regex::new(&format!(r"(?:[A-Za-z0-9_.@-]+/)*{dir}/[A-Za-z0-9_./@-]+\.md")).unwrap()
}

/// `first_line` is the 1-based line number of the body's first line in the file.
pub fn scan(body: &str, first_line: usize, path_re: &Regex) -> Body {
    let mut out = Body::default();
    let mut fence: Option<String> = None;
    for (i, raw_line) in body.lines().enumerate() {
        let line_no = first_line + i;
        out.lines += 1;
        let trimmed = raw_line.trim_start();
        if let Some(f) = &fence {
            if trimmed.starts_with(f.as_str())
                && trimmed.trim_start_matches(f.chars().next().unwrap()).trim().is_empty()
            {
                fence = None;
            }
            continue;
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            let ch = trimmed.chars().next().unwrap();
            let n = trimmed.chars().take_while(|c| *c == ch).count();
            fence = Some(std::iter::repeat_n(ch, n).collect());
            continue;
        }
        let line = inline_code_re().replace_all(raw_line, "");
        if let Some(rest) = trimmed.strip_prefix('#') {
            let level_rest = rest.trim_start_matches('#');
            if level_rest.starts_with(' ') || level_rest.is_empty() {
                let heading_text = level_rest.trim();
                let heading_clean = anchor_re().replace(heading_text, "").trim().to_string();
                if out.first_heading.is_none() && !heading_clean.is_empty() {
                    out.first_heading = Some(heading_clean.clone());
                }
                if let Some(c) = anchor_re().captures(heading_text) {
                    out.anchors.push(Anchor { name: c[1].to_string(), heading: heading_clean, line: line_no });
                }
            }
        }
        for c in wiki_re().captures_iter(&line) {
            out.links.push(Link { text: c[1].trim().to_string(), line: line_no });
        }
        for m in path_re.find_iter(&line) {
            out.path_cites.push(Link { text: m.as_str().to_string(), line: line_no });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_links_anchors_and_cites() {
        let body = "# Checkout\n\nSee [[adr:shop/20260101000000]] and [[sot:shop/cart#CO-1|the rule]].\n\n## Totals {#CO-1}\n\n```\n[[not:a/link]]\n```\nInline `[[not:a/link]]` too.\nOld path services/shop/.spec/plan/x.plan.md here.\n";
        let b = scan(body, 5, &path_cite_re(".spec"));
        assert_eq!(b.links.len(), 2);
        assert_eq!(b.links[0].line, 7);
        assert_eq!(b.links[1].text, "sot:shop/cart#CO-1");
        assert_eq!(b.anchors[0].name, "CO-1");
        assert_eq!(b.anchors[0].heading, "Totals");
        assert_eq!(b.first_heading.as_deref(), Some("Checkout"));
        assert_eq!(b.path_cites[0].text, "services/shop/.spec/plan/x.plan.md");
    }
}
