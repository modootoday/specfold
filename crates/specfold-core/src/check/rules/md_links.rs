//! L8: plain Markdown links and images in a body lead somewhere on disk, and their `#fragment`
//! names a heading (GitHub slug) or `{#anchor}`. L9: an image has alt text. Offline only:
//! links with a scheme (https:, mailto:, ids) are left to L1 and to link checkers.

use super::support::skipped;
use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::export::links::{MdLink, md_links};
use crate::markdown::frontmatter::split;
use std::collections::BTreeSet;

pub struct LocalLinks;
pub struct ImageAlt;

/// The fragments a file's headings answer to: GitHub slugs (a repeat gets `-1`, `-2`) and
/// explicit `{#anchor}` ids.
pub fn fragments(body: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut seen: std::collections::HashMap<String, usize> = Default::default();
    for (_, text) in crate::markdown::sections::outline(body) {
        let (text, anchor) = match text.rsplit_once("{#") {
            Some((t, a)) if a.ends_with('}') => (t.trim().to_string(), Some(a.trim_end_matches('}').to_string())),
            _ => (text, None),
        };
        if let Some(a) = anchor {
            out.insert(a);
        }
        let slug = crate::export::markdown::github_slug(&text);
        let n = seen.entry(slug.clone()).or_default();
        out.insert(if *n == 0 { slug.clone() } else { format!("{slug}-{n}") });
        *n += 1;
    }
    out
}

fn has_scheme(t: &str) -> bool {
    let head: String = t.chars().take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-')).collect();
    t.starts_with("//") || (!head.is_empty() && t[head.len()..].starts_with(':'))
}

fn decode(p: &str) -> String {
    p.replace("%20", " ")
}

/// `a/b/../c` as `a/c`; None when it climbs out of the repository.
fn normalize(p: &str) -> Option<String> {
    let mut parts: Vec<&str> = vec![];
    for seg in p.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    Some(parts.join("/"))
}

/// The links of every file of one document, with the line each sits on.
fn each_link(ctx: &Ctx, d: &crate::model::Doc, mut f: impl FnMut(&str, &str, usize, &MdLink)) {
    let files = std::iter::once(d.path.as_str()).chain(d.chapters.iter().map(String::as_str));
    for path in files {
        let Some(text) = ctx.current(path) else { continue };
        let sp = split(&text);
        for l in md_links(sp.body) {
            let line = sp.body_line + sp.body[..l.start].matches('\n').count();
            f(path, sp.body, line, &l);
        }
    }
}

impl Rule for LocalLinks {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "L8",
            title: "local Markdown links lead somewhere",
            explain: "A plain Markdown link or image with a relative path ([text](../api.md), ![](diagram.png)) must name a file or directory that exists, and a #fragment must match a heading of the target Markdown file (or of this file for [text](#heading)): its GitHub slug (lower case, spaces to -, punctuation dropped, a repeated heading gets -1, -2) or an explicit {#anchor}. Renaming a heading or moving a file breaks such links silently. Links with a scheme (https:, mailto:) and document ids are not checked here (L1 checks ids; use a link checker such as lychee for URLs). Fix the path or fragment, or write [[id#anchor]] for another document. Targets matching a glob in [check] link_ignore (as written, or as the repository path) are skipped. Warns; turn it off with [check.severity] L8 = \"off\".",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        let root = &ctx.corpus.cfg.root;
        let ignore: Vec<_> =
            ctx.corpus.cfg.raw.check.link_ignore.iter().filter_map(|p| crate::workspace::owners::matcher(p)).collect();
        let ignored = |s: &str| ignore.iter().any(|m| m.is_match(s));
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers(i) || skipped(d, "L8") {
                continue;
            }
            each_link(ctx, d, |path, body, line, l| {
                let t = l.target.trim();
                if t.is_empty() || has_scheme(t) || ignored(t) || ignored(t.split('#').next().unwrap_or(t)) {
                    return;
                }
                let (file, frag) = match t.split_once('#') {
                    Some((f, h)) => (f, Some(h)),
                    None => (t, None),
                };
                let file = decode(file.split('?').next().unwrap_or(""));
                let what = if l.image { "image" } else { "link" };
                let (rel, text) = if file.is_empty() {
                    (path.to_string(), Some(body.to_string()))
                } else {
                    let joined = match file.strip_prefix('/') {
                        Some(abs) => abs.to_string(),
                        None => format!("{}/{file}", path.rsplit_once('/').map_or("", |x| x.0)),
                    };
                    let rel = normalize(&joined);
                    if rel.as_deref().is_some_and(ignored) {
                        return;
                    }
                    let Some(rel) = rel else {
                        out.push(Finding::warn("L8", path, line, format!("{what} {t} leaves the repository")));
                        return;
                    };
                    if !root.join(&rel).exists() {
                        out.push(Finding::warn("L8", path, line, format!("{what} {t}: {rel} does not exist")));
                        return;
                    }
                    let md = rel.ends_with(".md") || rel.ends_with(".markdown");
                    let text = (md && frag.is_some())
                        .then(|| std::fs::read_to_string(root.join(&rel)).ok())
                        .flatten()
                        .map(|x| split(&x).body.to_string());
                    (rel, text)
                };
                let (Some(h), Some(text)) = (frag.filter(|h| !h.is_empty()), text) else { return };
                let h = decode(h);
                let known = fragments(&text);
                if !known.contains(&h) && !known.contains(&h.to_lowercase()) {
                    let near: Vec<&String> = known
                        .iter()
                        .filter(|k| k.contains(&h.to_lowercase()) || h.to_lowercase().contains(k.as_str()))
                        .collect();
                    let hint = match near.as_slice() {
                        [one] => format!("; did you mean #{one}?"),
                        _ => String::new(),
                    };
                    out.push(Finding::warn("L8", path, line, format!("{what} {t}: no heading #{h} in {rel}{hint}")));
                }
            });
        }
    }
}

impl Rule for ImageAlt {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "L9",
            title: "images have alt text",
            explain: "An image written ![](diagram.png) says nothing to a screen reader or to an agent reading the text. Describe what it shows: ![Checkout sequence: cart, payment, receipt](diagram.png). Warns; turn it off with [check.severity] L9 = \"off\".",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers(i) || skipped(d, "L9") {
                continue;
            }
            each_link(ctx, d, |path, _, line, l| {
                if l.image && l.text.trim().is_empty() {
                    out.push(Finding::warn("L9", path, line, format!("image {} has no alt text", l.target)));
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_repeat_and_anchors() {
        let f = fragments("# Title\n## Use files\n## Use files\n### Detail {#R-1}\n");
        for k in ["title", "use-files", "use-files-1", "detail", "R-1"] {
            assert!(f.contains(k), "{k} in {f:?}");
        }
        assert!(has_scheme("https://x") && has_scheme("mailto:a@b") && has_scheme("adr:o/1"));
        assert!(!has_scheme("../a.md") && !has_scheme("#x"));
        assert_eq!(normalize("a/b/../c"), Some("a/c".into()));
        assert_eq!(normalize("../x"), None);
    }
}
