//! The little git specfold needs, through the git command line. Every function degrades to
//! "unknown" when git is missing or the directory is not a repository.

use std::path::{Path, PathBuf};
use std::process::Command;

fn git(root: &Path, args: &[&str]) -> Option<Vec<u8>> {
    let out = Command::new("git").arg("-C").arg(root).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(out.stdout)
}

fn split_nul(bytes: Vec<u8>) -> Vec<String> {
    bytes.split(|b| *b == 0).filter(|s| !s.is_empty()).map(|s| String::from_utf8_lossy(s).into_owned()).collect()
}

pub fn git_root(dir: &Path) -> Option<PathBuf> {
    let out = git(dir, &["rev-parse", "--show-toplevel"])?;
    let s = String::from_utf8(out).ok()?;
    Some(PathBuf::from(s.trim()))
}

pub fn is_repo(dir: &Path) -> bool {
    git(dir, &["rev-parse", "--git-dir"]).is_some()
}

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

/// The upstream of the current branch, used as the default push range base.
pub fn upstream(root: &Path) -> Option<String> {
    let out = git(root, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}"])?;
    let s = String::from_utf8(out).ok()?.trim().to_string();
    if s.is_empty() { None } else { Some(s) }
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

pub struct Revision {
    pub hash: String,
    pub date: String,
    pub author: String,
    /// The file's path in that commit (it may have been renamed since).
    pub path: String,
}

/// Every commit that touched `path`, oldest first, following renames.
pub fn history(root: &Path, path: &str) -> Option<Vec<Revision>> {
    let out = git(root, &["log", "--follow", "--format=%x1e%H%x00%cs%x00%an", "--name-only", "--", path])?;
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
            let path = lines.find(|l| !l.trim().is_empty())?.trim().to_string();
            Some(Revision { hash, date, author, path })
        })
        .collect();
    revs.reverse();
    Some(revs)
}

/// The date of the first commit that added `path`, as `YYYYMMDDHHMMSS` UTC.
pub fn first_commit_stamp(root: &Path, path: &str) -> Option<String> {
    let out = git(root, &["log", "--diff-filter=A", "--follow", "--format=%at", "--", path])?;
    let text = String::from_utf8_lossy(&out);
    let secs: i64 = text.lines().last()?.trim().parse().ok()?;
    let t = time::OffsetDateTime::from_unix_timestamp(secs).ok()?;
    let f = time::macros::format_description!("[year][month][day][hour][minute][second]");
    t.format(f).ok()
}
