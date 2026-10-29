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
