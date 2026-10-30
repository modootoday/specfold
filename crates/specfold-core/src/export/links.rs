//! Every `[[reference]]` in a body with its byte span and what it resolves to, using the same
//! resolution as rule L1 (ids, aliases, paths, anchors and remotes).

use crate::config::Config;
use crate::corpus::Corpus;
use crate::graph::{Graph, Resolution};
use crate::markdown::body::wiki_re;
use crate::model::RefText;
use crate::remotes::{self, Remote};
use serde::Serialize;

/// One reference as written: `span` is the byte range of `[[…]]` in the body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Occurrence {
    pub span: (usize, usize),
    pub raw: String,
    pub target: String,
    pub label: Option<String>,
}

/// A reference and where it leads. `target_id` is set for a document here or in a remote;
/// `reason` is set when it does not resolve.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Link {
    pub span: (usize, usize),
    pub raw: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub target_id: Option<String>,
    pub anchor: Option<String>,
    pub remote: Option<String>,
    pub resolved_path: Option<String>,
    pub title: Option<String>,
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The local document index, for exporters inside specfold.
    #[serde(skip)]
    pub idx: Option<usize>,
}

/// The references in `body`, skipping fenced code blocks and inline code like rule L1 does.
pub fn occurrences(body: &str) -> Vec<Occurrence> {
    let mut out = Vec::new();
    outside_code(body, wiki_re(), |start, m| {
        let whole = m.get(0).expect("capture group 0 always exists");
        let inner = &whole.as_str()[2..whole.as_str().len() - 2];
        let label = inner.split_once('|').map(|(_, l)| l.trim().to_string()).filter(|l| !l.is_empty());
        out.push(Occurrence {
            span: (start + whole.start(), start + whole.end()),
            raw: whole.as_str().to_string(),
            target: m[1].trim().to_string(),
            label,
        });
    });
    out
}

/// `[text](plan:root/…)`: a Markdown link whose target has the shape of a document id. Such a
/// link is not a reference: rule L1 warns and points at `[[id]]`, and the built-in export
/// rewrites the ones that resolve.
fn md_id_re() -> &'static regex::Regex {
    static R: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    R.get_or_init(|| {
        regex::Regex::new(r"(^|[^!\]])\[([^\[\]\n]*)\]\(<?([a-z][a-z0-9_-]*:[^\s()<>/:]+/[^\s()<>]+)>?\)")
            .expect("static regex is valid")
    })
}

/// The Markdown links in `body` whose target looks like `<type>:<owner>/<rest>`, outside code.
pub fn md_id_occurrences(body: &str) -> Vec<Occurrence> {
    let mut out = Vec::new();
    outside_code(body, md_id_re(), |start, m| {
        let lead = m.get(1).map_or(0, |g| g.len());
        let whole = m.get(0).expect("capture group 0 always exists");
        let text = m[2].trim().to_string();
        out.push(Occurrence {
            span: (start + whole.start() + lead, start + whole.end()),
            raw: whole.as_str()[lead..].to_string(),
            target: m[3].to_string(),
            label: Some(text).filter(|l| !l.is_empty()),
        });
    });
    out
}

/// A plain Markdown link or image: `[text](target)` or `![alt](target "title")`.
#[derive(Debug, Clone, PartialEq)]
pub struct MdLink {
    /// Byte offset in the body.
    pub start: usize,
    pub image: bool,
    pub text: String,
    pub target: String,
}

fn md_link_re() -> &'static regex::Regex {
    static R: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    R.get_or_init(|| {
        regex::Regex::new(
            r#"(!?)\[([^\[\]\n]*)\]\(\s*(?:<([^<>\n]*)>|([^\s()<>]*))(?:\s+(?:"[^"\n]*"|'[^'\n]*'))?\s*\)"#,
        )
        .expect("static regex is valid")
    })
}

/// Every inline Markdown link and image in `body`, outside code.
pub fn md_links(body: &str) -> Vec<MdLink> {
    let mut out = Vec::new();
    outside_code(body, md_link_re(), |start, m| {
        let target = m.get(3).or(m.get(4)).map_or("", |g| g.as_str()).to_string();
        out.push(MdLink {
            start: start + m.get(0).expect("capture group 0 always exists").start(),
            image: !m[1].is_empty(),
            text: m[2].to_string(),
            target,
        });
    });
    out
}

/// Calls `each(line_start, captures)` for every match of `re` outside fences and inline code.
fn outside_code(body: &str, re: &regex::Regex, mut each: impl FnMut(usize, &regex::Captures)) {
    let mut fence = crate::markdown::Fence::default();
    let mut offset = 0;
    for line in body.split_inclusive('\n') {
        let start = offset;
        offset += line.len();
        let trimmed = line.trim_start();
        if fence.step(trimmed) || fence.inside() {
            continue;
        }
        let code = code_spans(line);
        for m in re.captures_iter(line) {
            let whole = m.get(0).expect("capture group 0 always exists");
            if code.iter().any(|&(a, b)| whole.start() < b && whole.end() > a) {
                continue;
            }
            each(start, &m);
        }
    }
}

/// Byte ranges of inline code on one line.
fn code_spans(line: &str) -> Vec<(usize, usize)> {
    let b = line.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'`' {
            i += 1;
            continue;
        }
        let n = b[i..].iter().take_while(|&&c| c == b'`').count();
        let open = i;
        i += n;
        let mut j = i;
        let mut closed = false;
        while j < b.len() {
            if b[j] == b'`' {
                let m = b[j..].iter().take_while(|&&c| c == b'`').count();
                if m == n {
                    out.push((open, j + m));
                    i = j + m;
                    closed = true;
                    break;
                }
                j += m;
            } else {
                j += 1;
            }
        }
        if !closed {
            break;
        }
    }
    out
}

/// Resolves one reference.
pub fn resolve(c: &Corpus, g: &Graph, o: &Occurrence) -> Link {
    let r = RefText::parse(&o.target);
    let mut l = Link {
        span: o.span,
        raw: o.raw.clone(),
        label: o.label.clone(),
        target_id: None,
        anchor: r.anchor,
        remote: None,
        resolved_path: None,
        title: None,
        status: None,
        reason: None,
        idx: None,
    };
    match g.resolve(&o.target) {
        Resolution::Doc { idx, anchor, .. } => {
            let d = &c.docs[idx];
            l.target_id = Some(d.id.to_string());
            l.anchor = anchor;
            l.resolved_path = Some(d.path.clone());
            l.title = d.title.clone();
            l.status = d.status();
            l.idx = Some(idx);
        }
        Resolution::Ambiguous(list) => {
            l.reason = Some(format!("ambiguous ({} documents)", list.len()));
        }
        Resolution::MissingAnchor { idx, anchor } => {
            l.target_id = Some(c.docs[idx].id.to_string());
            l.resolved_path = Some(c.docs[idx].path.clone());
            l.reason = Some(format!("{} has no anchor #{anchor}", c.docs[idx].id));
        }
        Resolution::Missing => remote(&c.cfg, &o.target, &mut l),
    }
    l
}

fn remote(cfg: &Config, text: &str, l: &mut Link) {
    l.reason = Some(match remotes::resolve(cfg, text) {
        Remote::Local => "names no document".into(),
        Remote::Found { name, id, title, file, .. } => {
            l.target_id = Some(id);
            l.remote = Some(name);
            l.title = Some(title).filter(|t| !t.is_empty());
            l.resolved_path = Some(file.display().to_string());
            return;
        }
        Remote::Absent { name, .. } => {
            l.remote = Some(name.clone());
            format!("remote {name} is not on disk")
        }
        Remote::Missing { name, id } => {
            l.remote = Some(name.clone());
            format!("{id} is not in remote {name}")
        }
        Remote::MissingAnchor { name, id, anchor } => {
            l.remote = Some(name);
            format!("{id} has no anchor #{anchor}")
        }
        Remote::Ambiguous { name, ids } => {
            l.remote = Some(name);
            format!("ambiguous ({} documents)", ids.len())
        }
    });
}

/// Every reference in `body`, resolved.
/// Markdown links to a document id count as references here when they resolve, so an export
/// rewrites them; one that resolves to nothing is left as written.
pub fn of_body(c: &Corpus, g: &Graph, body: &str) -> Vec<Link> {
    let mut out: Vec<Link> = occurrences(body).iter().map(|o| resolve(c, g, o)).collect();
    out.extend(
        md_id_occurrences(body).iter().map(|o| resolve(c, g, o)).filter(|l| l.idx.is_some() && l.remote.is_none()),
    );
    out.sort_by_key(|l| l.span.0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_skip_code() {
        let body = "See [[a:b/c|the c]] and `[[x:y]]`.\n```\n[[no:pe]]\n```\nAnd [[d:e#F]].\n";
        let o = occurrences(body);
        assert_eq!(o.len(), 2);
        assert_eq!(&body[o[0].span.0..o[0].span.1], "[[a:b/c|the c]]");
        assert_eq!(o[0].target, "a:b/c");
        assert_eq!(o[0].label.as_deref(), Some("the c"));
        assert_eq!(&body[o[1].span.0..o[1].span.1], "[[d:e#F]]");
    }

    #[test]
    fn plain_links_and_images() {
        let b = "See [a](x.md#Top) and ![](d.png \"t\"), `[no](code)`.\n```\n[no](fence)\n```\n![alt](<a b.png>)\n";
        let l = md_links(b);
        let t: Vec<(&str, bool, &str)> = l.iter().map(|x| (x.target.as_str(), x.image, x.text.as_str())).collect();
        assert_eq!(t, [("x.md#Top", false, "a"), ("d.png", true, ""), ("a b.png", true, "alt")]);
    }

    #[test]
    fn markdown_links_shaped_like_ids() {
        let body = "See [the plan](plan:root/2026) and [x](https://a.b/c), ![i](img:a/b),\n`[c](plan:a/b)` [d](<adr:shop/x#Y>).\n";
        let o = md_id_occurrences(body);
        assert_eq!(o.len(), 2, "{o:?}");
        assert_eq!(&body[o[0].span.0..o[0].span.1], "[the plan](plan:root/2026)");
        assert_eq!(o[0].target, "plan:root/2026");
        assert_eq!(o[0].label.as_deref(), Some("the plan"));
        assert_eq!(o[1].target, "adr:shop/x#Y");
    }
}
