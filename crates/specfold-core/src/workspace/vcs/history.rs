//! History: commits in a range, a file's revisions and when governed code last changed.

use super::repo::{git, nearest_repo};
use std::path::Path;

pub struct CommitText {
    pub hash: String,
    pub author: String,
    pub message: String,
}

/// Commits in a range with their author and message.
pub fn commits(root: &Path, range: &str) -> Option<Vec<CommitText>> {
    let out = git(root, &["log", "--format=%H%x00%an <%ae>%x00%B%x1e", range])?;
    let text = String::from_utf8_lossy(&out).into_owned();
    Some(
        text.split('\u{1e}')
            .filter_map(|rec| {
                let rec = rec.trim_start_matches('\n');
                let mut parts = rec.splitn(3, '\0');
                let hash = parts.next()?.to_string();
                if hash.is_empty() {
                    return None;
                }
                Some(CommitText {
                    hash,
                    author: parts.next()?.to_string(),
                    message: parts.next().unwrap_or("").to_string(),
                })
            })
            .collect(),
    )
}

/// The date (YYYY-MM-DD) of the last commit touching any of `paths`, which may be globs.
pub fn last_change(root: &Path, paths: &[String]) -> Option<String> {
    let mut args: Vec<String> = vec!["log".into(), "-1".into(), "--format=%cs".into(), "--".into()];
    args.extend(paths.iter().map(|p| if p.contains(['*', '?', '[']) { format!(":(glob){p}") } else { p.clone() }));
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = git(root, &refs)?;
    let s = String::from_utf8_lossy(&out).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

/// Commits after `since` up to HEAD touching any of `paths` (globs allowed) outside `exclude`
/// (globs), newest first, as short hashes; None when `since` is not a commit git knows.
pub fn commits_since(root: &Path, since: &str, paths: &[String], exclude: &[String]) -> Option<Vec<String>> {
    git(root, &["cat-file", "-e", &format!("{since}^{{commit}}")])?;
    let range = format!("{since}..HEAD");
    let mut args: Vec<String> = vec!["log".into(), "--format=%h".into(), range, "--".into()];
    args.extend(paths.iter().map(|p| if p.contains(['*', '?', '[']) { format!(":(glob){p}") } else { p.clone() }));
    args.extend(exclude.iter().map(|e| format!(":(exclude,glob){e}")));
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = git(root, &refs)?;
    Some(String::from_utf8_lossy(&out).lines().map(str::to_string).filter(|l| !l.is_empty()).collect())
}

pub struct Revision {
    pub hash: String,
    pub date: String,
    pub author: String,
    /// The file's path in that commit (it may have been renamed since).
    pub path: String,
}

/// Every commit that touched `path`, oldest first, following renames.
pub fn history(root: &Path, path: &str) -> Option<Vec<Revision>> {
    let (repo, rel, prefix) = nearest_repo(root, path);
    let out = git(&repo, &["log", "--follow", "--format=%x1e%H%x00%cs%x00%an", "--name-only", "--", &rel])?;
    let text = String::from_utf8_lossy(&out).into_owned();
    let mut revs: Vec<Revision> = text
        .split('\u{1e}')
        .filter_map(|rec| {
            let mut lines = rec.lines();
            let head = lines.next()?;
            let mut parts = head.split('\0');
            let hash = parts.next()?.to_string();
            let date = parts.next()?.to_string();
            let author = parts.next().unwrap_or("").to_string();
            let path = format!("{prefix}{}", lines.find(|l| !l.trim().is_empty())?.trim());
            Some(Revision { hash, date, author, path })
        })
        .collect();
    revs.reverse();
    Some(revs)
}
