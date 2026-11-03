//! Unit tests for source revisions and section changes.

use super::rev::{rev_of_parts, rev_of_text, section_changes};
use super::types::Part;

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
