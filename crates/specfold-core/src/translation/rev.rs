//! A source's revision (`source_rev`): its normalized body hashed, read now or from git, and the
//! sections that changed between two revisions.

use super::types::{Part, SectionChange};
use crate::corpus::Corpus;
use crate::corpus::scan::hex;
use crate::markdown::frontmatter;
use sha2::{Digest, Sha256};

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

/// Sections of one body: heading text and normalized paragraphs, fenced code kept whole.
fn sections(label: &str, body: &str) -> Vec<(String, Vec<String>)> {
    let name = |h: &str| if label.is_empty() { h.to_string() } else { format!("{label}: {h}") };
    let mut out: Vec<(String, Vec<String>)> = vec![(name("(top)"), vec![])];
    let mut para: Vec<&str> = vec![];
    let mut fence = crate::markdown::Fence::default();
    let flush = |para: &mut Vec<&str>, out: &mut Vec<(String, Vec<String>)>| {
        let t = para.iter().flat_map(|l| l.split_whitespace()).collect::<Vec<_>>().join(" ");
        if !t.is_empty() {
            out.last_mut().expect("one section").1.push(t);
        }
        para.clear();
    };
    for line in body.lines() {
        let t = line.trim_start();
        if fence.step(t) {
            para.push(line);
            continue;
        }
        if let Some((_, h)) = crate::markdown::atx(line).filter(|_| !fence.inside()) {
            flush(&mut para, &mut out);
            out.push((name(h), vec![]));
        } else if !fence.inside() && t.is_empty() {
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

/// Why a `source_rev` value is not `sha256:<12 hex>`, or `None` when it is.
pub fn rev_problem(rev: &str) -> Option<String> {
    let ok = rev
        .strip_prefix("sha256:")
        .is_some_and(|h| h.len() == 12 && h.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)));
    (!ok).then(|| format!("source_rev '{rev}' is not sha256:<12 hex>"))
}
