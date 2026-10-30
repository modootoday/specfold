//! What a change touches: paths in the index, since a base or in a range, the lines it adds,
//! and file contents at a revision or in the index.

use super::repo::{git, split_nul};
use std::collections::HashMap;
use std::path::Path;

/// Paths added, copied, modified or renamed in the index.
pub fn staged(root: &Path) -> Option<Vec<String>> {
    git(root, &["diff", "--cached", "--name-only", "-z", "--diff-filter=ACMR"]).map(split_nul)
}

/// Paths changed between `base` and the working tree, plus untracked files.
pub fn changed_since(root: &Path, base: &str) -> Option<Vec<String>> {
    let mut files = split_nul(git(root, &["diff", "--name-only", "-z", "--diff-filter=ACMR", base])?);
    if let Some(untracked) = git(root, &["ls-files", "-z", "--others", "--exclude-standard"]) {
        files.extend(split_nul(untracked));
    }
    files.sort();
    files.dedup();
    Some(files)
}

/// Paths changed in a commit range such as `origin/main..HEAD`.
pub fn changed_in_range(root: &Path, range: &str) -> Option<Vec<String>> {
    git(root, &["diff", "--name-only", "-z", "--diff-filter=ACMR", range]).map(split_nul)
}

/// Paths a change removes: deleted files and the old side of renames, in the index (`base`
/// None) or since `base`. They no longer exist, so the other change lists leave them out.
pub fn gone(root: &Path, base: Option<&str>) -> Option<Vec<String>> {
    let mut args = vec!["diff", "--name-status", "-z", "-M", "--diff-filter=DR"];
    match base {
        Some(b) => args.push(b),
        None => args.push("--cached"),
    }
    let words = split_nul(git(root, &args)?);
    let mut out = vec![];
    let mut it = words.into_iter();
    while let Some(status) = it.next() {
        let Some(old) = it.next() else { break };
        if status.starts_with('R') {
            it.next();
        }
        out.push(old);
    }
    Some(out)
}

/// Line numbers added per path: in the index against HEAD (`base` None) or in the working tree
/// against `base`. Lets a check judge only what a change writes, not the history it touches.
pub fn added_lines(root: &Path, base: Option<&str>) -> Option<HashMap<String, Vec<usize>>> {
    let mut args = vec!["diff", "-U0", "--no-color", "--no-ext-diff", "--diff-filter=ACMR"];
    match base {
        Some(b) => args.push(b),
        None => args.push("--cached"),
    }
    let out = git(root, &args)?;
    let mut map: HashMap<String, Vec<usize>> = HashMap::new();
    let mut current: Option<String> = None;
    for line in String::from_utf8_lossy(&out).lines() {
        if let Some(p) = line.strip_prefix("+++ ") {
            current = p.strip_prefix("b/").map(str::to_string);
        } else if let (Some(p), Some(h)) = (&current, line.strip_prefix("@@ ")) {
            let added = h.split(' ').find(|s| s.starts_with('+')).unwrap_or("+0,0");
            let (start, count) = added[1..].split_once(',').unwrap_or((&added[1..], "1"));
            let (start, count): (usize, usize) = (start.parse().unwrap_or(0), count.parse().unwrap_or(0));
            map.entry(p.clone()).or_default().extend(start..start + count);
        }
    }
    Some(map)
}

/// The content of `path` at `rev`, if it existed there.
pub fn show(root: &Path, rev: &str, path: &str) -> Option<String> {
    let spec = format!("{rev}:{path}");
    git(root, &["show", &spec]).map(|b| String::from_utf8_lossy(&b).into_owned())
}

/// The staged content of `path`.
pub fn show_staged(root: &Path, path: &str) -> Option<String> {
    let spec = format!(":{path}");
    git(root, &["show", &spec]).map(|b| String::from_utf8_lossy(&b).into_owned())
}
