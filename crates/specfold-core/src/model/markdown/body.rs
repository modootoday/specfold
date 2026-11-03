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

/// `[[target]]` and `[[target|label]]`; group 1 is the target. Callers keep only the matches
/// `is_wiki_link` accepts, or use `wiki_links`.
pub fn wiki_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\[\[([^\[\]\n|]+)(?:\|[^\[\]\n]*)?\]\]").expect("static regex is valid"))
}

/// Operators that, after a space, make a `[[…]]` a bash test.
const SHELL_OPERATORS: &[&str] = &["==", "!=", "=~", "-eq", "-ne", "-lt", "-gt", "-le", "-ge", "&&", "||"];

/// Whether a `wiki_re` match is a reference. Bash tests share the brackets (`[[ -f x ]]`,
/// `[[ $a == b ]]`) in comments and scripts; ids hold no spaces, so such a test never names one.
pub fn is_wiki_link(c: &regex::Captures) -> bool {
    let target = &c[1];
    if target.starts_with(|ch: char| ch.is_whitespace() || matches!(ch, '-' | '$' | '!')) {
        return false;
    }
    // The target stops at the first pipe, so `a || b` leaves `| b` to the label part.
    let or = c[0].contains(" || ");
    !or && !target.split_whitespace().skip(1).any(|w| SHELL_OPERATORS.contains(&w))
}

/// The references on one line: `wiki_re` matches that `is_wiki_link` accepts.
pub fn wiki_links(line: &str) -> impl Iterator<Item = regex::Captures<'_>> {
    wiki_re().captures_iter(line).filter(is_wiki_link)
}

fn anchor_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\{#([A-Za-z0-9][A-Za-z0-9_.-]*)\}\s*$").expect("static regex is valid"))
}

/// C201: a line with its inline code spans taken out, CommonMark style: a run of N backticks
/// closes only at the next run of exactly N; a run with no partner is plain text.
pub fn strip_code_spans(line: &str) -> String {
    let b = line.as_bytes();
    let run_at = |i: usize| b[i..].iter().take_while(|c| **c == b'`').count();
    let (mut out, mut i, mut kept) = (String::new(), 0, 0);
    while i < b.len() {
        if b[i] != b'`' {
            i += 1;
            continue;
        }
        let n = run_at(i);
        let mut j = i + n;
        let close = loop {
            match b[j..].iter().position(|c| *c == b'`') {
                None => break None,
                Some(p) => {
                    let at = j + p;
                    let m = run_at(at);
                    if m == n {
                        break Some(at + m);
                    }
                    j = at + m;
                }
            }
        };
        match close {
            Some(end) => {
                out.push_str(&line[kept..i]);
                kept = end;
                i = end;
            }
            None => i += n,
        }
    }
    out.push_str(&line[kept..]);
    out
}

/// Builds the regex that finds plain-text paths into `<spec_dir>/` ending in `.md`.
pub fn path_cite_re(spec_dir: &str) -> Regex {
    path_cite_re_with(spec_dir, &[])
}

/// [`path_cite_re`], also finding `<dir>/….<type>.md` for each `(dir, type)` kept in a directory
/// of its own. Such a directory is an ordinary name, so only document file names count there.
pub fn path_cite_re_with(spec_dir: &str, type_dirs: &[(String, String)]) -> Regex {
    let dir = regex::escape(spec_dir);
    let mut tail = format!(r"{dir}/[A-Za-z0-9_./@-]+\.md");
    if !type_dirs.is_empty() {
        let own =
            type_dirs.iter().map(|(d, t)| format!(r"{}/[A-Za-z0-9_./@-]+\.{}\.md", regex::escape(d), regex::escape(t)));
        tail = format!("(?:{tail}|{})", own.collect::<Vec<_>>().join("|"));
    }
    Regex::new(&format!(r"(?:[A-Za-z0-9_.@-]+/)*{tail}")).expect("escaped directory names always form a valid regex")
}

/// `first_line` is the 1-based line number of the body's first line in the file.
pub fn scan(body: &str, first_line: usize, path_re: &Regex) -> Body {
    let mut out = Body::default();
    let mut fence = crate::model::markdown::Fence::default();
    for (i, raw_line) in body.lines().enumerate() {
        let line_no = first_line + i;
        out.lines += 1;
        let trimmed = raw_line.trim_start();
        if fence.step(trimmed) || fence.inside() {
            continue;
        }
        let line = strip_code_spans(raw_line);
        if let Some((_, heading_text)) = crate::model::markdown::atx(raw_line) {
            let heading_clean = anchor_re().replace(heading_text, "").trim().to_string();
            if out.first_heading.is_none() && !heading_clean.is_empty() {
                out.first_heading = Some(heading_clean.clone());
            }
            if let Some(c) = anchor_re().captures(heading_text) {
                out.anchors.push(Anchor { name: c[1].to_string(), heading: heading_clean, line: line_no });
            }
        }
        for c in wiki_links(&line) {
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
    #[test]
    fn c201_code_spans_pair_equal_backtick_runs() {
        use super::strip_code_spans as s;
        assert_eq!(s("Also ``c ` d`` with [[x]] and `y`."), "Also  with [[x]] and .");
        assert_eq!(s("`` `id` ``"), "");
        assert_eq!(s("a ` b [[x]]"), "a ` b [[x]]", "an unpaired run is text");
        assert_eq!(s("`a` ``b"), " ``b");
    }

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

    #[test]
    fn bash_test_brackets_are_not_links() {
        let tests = [
            "[[ -f x ]]",
            "[[ $a == b ]]",
            "[[-n $a]]",
            "[[$a]]",
            "[[!x]]",
            "[[a != b]]",
            "[[a =~ ^b]]",
            "[[a -eq 1]]",
            "[[a -ge 1]]",
            "[[a && b]]",
            "[[a || b]]",
            "[[ ]]",
        ];
        for t in tests {
            assert_eq!(wiki_links(t).count(), 0, "{t}");
        }
        let links = ["[[adr:shop/1]]", "[[sot:shop/cart#CO-1|a == b]]", "[[x-eq]]", "[[a==b]]"];
        for t in links {
            assert_eq!(wiki_links(t).count(), 1, "{t}");
        }
    }
}
