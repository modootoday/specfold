//! A staged gitlink's documents as the index pins them: a submodule checked out at another commit
//! holds other text than the commit being recorded.

use super::repo::{git_inner, split_nul};
use super::submodules::{Side, expand};
use std::path::Path;

/// `(repository-relative path, text at the staged pin)` of every Markdown file inside a gitlink the
/// index moves whose submodule checkout differs from that pin (changed or deleted there).
pub fn staged_pin_texts(root: &Path) -> Vec<(String, String)> {
    let mut out = vec![];
    for m in expand(root, Side::Staged).moves {
        let sub = root.join(&m.path);
        let Some(listed) = git_inner(&sub, &["diff", "--name-only", "-z", "--diff-filter=MD", &m.new]) else {
            continue;
        };
        for rel in split_nul(listed).into_iter().filter(|p| p.ends_with(".md")) {
            if let Some(b) = git_inner(&sub, &["show", &format!("{}:{rel}", m.new)]) {
                out.push((format!("{}/{rel}", m.path), String::from_utf8_lossy(&b).into_owned()));
            }
        }
    }
    out
}
