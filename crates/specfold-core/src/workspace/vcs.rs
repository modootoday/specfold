//! The little git specfold needs, through the git command line. Every function degrades to
//! "unknown" when git is missing or the directory is not a repository.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

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

/// The repository that holds `path`: a submodule or nested checkout below `root` keeps its own
/// history, which the outer repository cannot see. Returns that directory, the path relative to
/// it, and the prefix between `root` and it.
fn nearest_repo(root: &Path, path: &str) -> (PathBuf, String, String) {
    let mut prefix = Path::new(path).parent();
    while let Some(dir) = prefix.filter(|d| !d.as_os_str().is_empty()) {
        if root.join(dir).join(".git").exists() {
            let p = dir.to_string_lossy().replace('\\', "/");
            let rel = path.strip_prefix(&format!("{p}/")).unwrap_or(path).to_string();
            return (root.join(dir), rel, format!("{p}/"));
        }
        prefix = dir.parent();
    }
    (root.to_path_buf(), path.to_string(), String::new())
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

/// When each path was first added, carried across renames, from one pass over the history.
/// Asking git per file with --follow costs seconds each in a long history.
fn first_adds(repo: &Path) -> HashMap<String, i64> {
    let mut seen = HashMap::new();
    let Some(out) =
        git(repo, &["log", "--reverse", "-M", "--diff-filter=AR", "--format=%x1e%at", "--name-status", "-z"])
    else {
        return seen;
    };
    let text = String::from_utf8_lossy(&out);
    for rec in text.split('\u{1e}').filter(|r| !r.is_empty()) {
        let mut fields = rec.split('\0').map(|f| f.trim_start_matches('\n'));
        let Some(secs) = fields.next().and_then(|h| h.trim().parse::<i64>().ok()) else { continue };
        while let Some(status) = fields.next().filter(|s| !s.is_empty()) {
            if status.starts_with('R') {
                let (Some(from), Some(to)) = (fields.next(), fields.next()) else { break };
                let first = seen.get(from).copied().unwrap_or(secs);
                seen.entry(to.to_string()).or_insert(first);
            } else if let Some(p) = fields.next() {
                seen.entry(p.to_string()).or_insert(secs);
            }
        }
    }
    seen
}

fn first_add_secs(repo: &Path, rel: &str) -> Option<i64> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, HashMap<String, i64>>>> = OnceLock::new();
    let mut cache = CACHE.get_or_init(Default::default).lock().ok()?;
    cache.entry(repo.to_path_buf()).or_insert_with(|| first_adds(repo)).get(rel).copied()
}

/// The date of the first commit that added `path`, as `YYYYMMDDHHMMSS` UTC.
pub fn first_commit_stamp(root: &Path, path: &str) -> Option<String> {
    let (repo, rel, _) = nearest_repo(root, path);
    let secs = first_add_secs(&repo, &rel)?;
    let t = time::OffsetDateTime::from_unix_timestamp(secs).ok()?;
    let f = time::macros::format_description!("[year][month][day][hour][minute][second]");
    t.format(f).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_checkout_owns_its_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let r = tmp.path();
        std::fs::create_dir_all(r.join("libs/sub/.git")).unwrap();
        std::fs::create_dir_all(r.join("libs/sub/docs")).unwrap();
        let (repo, rel, prefix) = nearest_repo(r, "libs/sub/docs/a.md");
        assert_eq!((repo, rel.as_str(), prefix.as_str()), (r.join("libs/sub"), "docs/a.md", "libs/sub/"));
        let (repo, rel, prefix) = nearest_repo(r, "docs/a.md");
        assert_eq!((repo, rel.as_str(), prefix.as_str()), (r.to_path_buf(), "docs/a.md", ""));
    }
}
