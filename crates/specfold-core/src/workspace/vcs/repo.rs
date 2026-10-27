//! Running git, and finding the repository that holds a path.

use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) fn git(root: &Path, args: &[&str]) -> Option<Vec<u8>> {
    let out = Command::new("git").arg("-C").arg(root).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(out.stdout)
}

pub(super) fn split_nul(bytes: Vec<u8>) -> Vec<String> {
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

/// The repository that holds `path`: a submodule or nested checkout below `root` keeps its own
/// history, which the outer repository cannot see. Returns that directory, the path relative to
/// it, and the prefix between `root` and it.
pub(super) fn nearest_repo(root: &Path, path: &str) -> (PathBuf, String, String) {
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
