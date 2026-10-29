//! Small helpers shared by rules.

use crate::check::context::Ctx;
use crate::model::Doc;

/// Frontmatter keys every document may carry.
pub const COMMON_KEYS: &[&str] = &[
    "id",
    "title",
    "status",
    "created",
    "updated",
    "owners",
    "tags",
    "aliases",
    "rel",
    "files",
    "generated_by",
    "check_skip",
    "verified_at",
];
pub const REQUIRED_KEYS: &[&str] = &["id", "title", "status", "created", "updated"];

/// The 1-based line of a top-level frontmatter key in a file, or 1.
pub fn key_line(ctx: &Ctx, path: &str, key: &str) -> usize {
    let Some(text) = ctx.current(path) else { return 1 };
    let prefix = format!("{key}:");
    text.lines().position(|l| l.starts_with(&prefix)).map(|i| i + 1).unwrap_or(1)
}

/// Rules a document opts out of with `check_skip: [L5, …]`, each needing a reason in the body.
pub fn skipped(doc: &Doc, rule: &str) -> bool {
    doc.list_field("check_skip").iter().any(|r| r == rule)
}

pub fn quote(s: &str) -> String {
    if s.chars().all(|c| c.is_ascii_alphanumeric() || "-_./:#@".contains(c)) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

/// Edit distance between two strings, by characters.
pub(super) fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for j in 0..b.len() {
            let cur = row[j + 1];
            row[j + 1] = (prev + usize::from(ca != b[j])).min(row[j] + 1).min(cur + 1);
            prev = cur;
        }
    }
    row[b.len()]
}

/// The one value close enough to `want` to be a typo of it: within a third of its length (at
/// least two edits), and strictly closer than any other value.
pub(super) fn nearest<'a>(want: &str, values: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let mut scored: Vec<(usize, &str)> = values
        .map(|v| (distance(&want.to_lowercase(), &v.to_lowercase()), v))
        .filter(|(d, v)| *d > 0 && *d <= 2.max(v.chars().count() / 3))
        .collect();
    scored.sort();
    match scored.as_slice() {
        [(d, v), rest @ ..] if rest.first().is_none_or(|(e, _)| e > d) => Some(v),
        _ => None,
    }
}
