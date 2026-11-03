//! The real form of an absolute path that may not exist yet: its deepest existing part is
//! resolved on disk (symbolic links included), and the missing rest is appended to it.

use std::path::{Component, Path, PathBuf};

/// `abs` with its nearest existing ancestor resolved and the missing names appended as written.
/// `None` when a missing part is `..` (it cannot be judged without the folder) or nothing exists.
pub fn real_with_missing_tail(abs: &Path) -> Option<PathBuf> {
    if let Ok(a) = abs.canonicalize() {
        return Some(a);
    }
    let mut tail = Vec::new();
    let mut cur = abs;
    while let Some(parent) = cur.parent() {
        let name = cur.file_name()?;
        tail.push(name.to_os_string());
        if let Ok(base) = parent.canonicalize() {
            return Some(tail.iter().rev().fold(base, |p, c| p.join(c)));
        }
        cur = parent;
    }
    None
}

/// Like [`real_with_missing_tail`], for a path compared where a write would land: a `..` in the
/// missing part removes the name before it instead of giving up. `None` when nothing exists.
pub fn real_with_lexical_tail(abs: &Path) -> Option<PathBuf> {
    let comps: Vec<_> = abs.components().collect();
    for k in (1..=comps.len()).rev() {
        let Ok(mut out) = comps[..k].iter().collect::<PathBuf>().canonicalize() else { continue };
        for c in &comps[k..] {
            match c {
                Component::ParentDir => {
                    out.pop();
                }
                Component::Normal(n) => out.push(n),
                _ => {}
            }
        }
        return Some(out);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_tails_follow_the_real_folder() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join("real/sub")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.join("real"), root.join("lnk")).unwrap();
        let mut cases: Vec<(&str, Option<PathBuf>, Option<PathBuf>)> = vec![
            ("real/sub", Some(root.join("real/sub")), Some(root.join("real/sub"))),
            ("real/new/file.md", Some(root.join("real/new/file.md")), Some(root.join("real/new/file.md"))),
            ("real/sub/../x", Some(root.join("real/x")), Some(root.join("real/x"))),
            ("real/new/../x", None, Some(root.join("real/x"))),
            ("real/new/..", None, Some(root.join("real"))),
        ];
        if cfg!(unix) {
            cases.push(("lnk/sub/new.md", Some(root.join("real/sub/new.md")), Some(root.join("real/sub/new.md"))));
            cases.push(("lnk/new/..", None, Some(root.join("real"))));
        }
        for (rel, missing, lexical) in cases {
            let abs = root.join(rel);
            assert_eq!(real_with_missing_tail(&abs), missing, "missing tail of {rel}");
            assert_eq!(real_with_lexical_tail(&abs), lexical, "lexical tail of {rel}");
        }
    }

    #[test]
    fn relative_paths_with_nothing_on_disk_have_no_real_form() {
        let p = Path::new("no-such-dir-anywhere/x/y");
        assert_eq!(real_with_missing_tail(p), None);
        assert_eq!(real_with_lexical_tail(p), None);
    }
}
