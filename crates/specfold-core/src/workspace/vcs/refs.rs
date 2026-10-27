//! Revisions: HEAD, the upstream branch, and whether a commit is in HEAD's history.

use super::repo::git;
use std::path::Path;
use std::process::{Command, Stdio};

/// The full hash of HEAD.
pub fn head(root: &Path) -> Option<String> {
    let out = git(root, &["rev-parse", "HEAD"])?;
    Some(String::from_utf8_lossy(&out).trim().to_string())
}

/// The upstream of the current branch, used as the default push range base.
pub fn upstream(root: &Path) -> Option<String> {
    let out = git(root, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}"])?;
    let s = String::from_utf8(out).ok()?.trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

/// Whether `rev` is HEAD or one of its ancestors; None when git cannot tell (unknown revision).
pub fn is_ancestor_of_head(root: &Path, rev: &str) -> Option<bool> {
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["merge-base", "--is-ancestor", rev, "HEAD"])
        .stderr(Stdio::null())
        .status()
        .ok()?;
    match status.code() {
        Some(0) => Some(true),
        Some(1) => Some(false),
        _ => None,
    }
}
