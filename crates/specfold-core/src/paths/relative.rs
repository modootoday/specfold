//! Paths written relative to something else: a link between two files, or a path below a root
//! with `/` separators on every platform.

use std::path::Path;

/// `to` written relative to the folder `dir`: `a/x` and `a/b/c.md` give `../b/c.md`.
pub fn relative_from_dir(dir: &str, to: &str) -> String {
    let base: Vec<&str> = dir.split('/').filter(|p| !p.is_empty()).collect();
    let target: Vec<&str> = to.split('/').collect();
    let common = base.iter().zip(&target).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<&str> = vec![".."; base.len() - common];
    parts.extend(&target[common..]);
    parts.join("/")
}

/// `to` written relative to the folder holding the file `from`.
pub fn relative_from_file(from: &str, to: &str) -> String {
    relative_from_dir(from.rsplit_once('/').map(|x| x.0).unwrap_or(""), to)
}

/// A path as text with `/` separators.
pub fn slash(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// `abs` relative to `root` with `/` separators (empty for the root itself); `None` outside it.
pub fn rel_of(root: &Path, abs: &Path) -> Option<String> {
    abs.strip_prefix(root).ok().map(slash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_from_dir_climbs_to_the_common_folder() {
        let cases: &[(&str, &str, &str)] = &[
            (".spec/adr", ".spec/adr/a.adr.md", "a.adr.md"),
            ("svc/.spec/adr", ".spec/adr/a.md", "../../../.spec/adr/a.md"),
            ("a/x", "a/b/c.md", "../b/c.md"),
            ("", "a/b.md", "a/b.md"),
            ("a/", "a/b.md", "b.md"),
            ("a/b", "a", ".."),
        ];
        for (dir, to, want) in cases {
            assert_eq!(relative_from_dir(dir, to), *want, "{dir} -> {to}");
        }
    }

    #[test]
    fn relative_from_file_starts_at_the_files_folder() {
        let cases: &[(&str, &str, &str)] = &[
            ("shop/adr/a.md", "shop/sot/b.md", "../sot/b.md"),
            ("shop/adr/a.md", "shop/adr/b.md", "b.md"),
            ("a.md", "docs/b.md", "docs/b.md"),
            ("docs/a.md", "b.md", "../b.md"),
            ("x/y/a.md", "x/z/w/b.md", "../z/w/b.md"),
        ];
        for (from, to, want) in cases {
            assert_eq!(relative_from_file(from, to), *want, "{from} -> {to}");
        }
    }

    #[test]
    fn rel_of_strips_the_root_and_uses_slashes() {
        let root = Path::new("/r/repo");
        let cases: &[(&str, Option<&str>)] = &[
            ("/r/repo/a/b.md", Some("a/b.md")),
            ("/r/repo", Some("")),
            ("/r/repo-x/a.md", None),
            ("/elsewhere/a.md", None),
            ("/r/repo/a\\b.md", Some("a/b.md")),
        ];
        for (abs, want) in cases {
            assert_eq!(rel_of(root, Path::new(abs)).as_deref(), *want, "{abs}");
        }
        assert_eq!(slash(Path::new("a\\b/c")), "a/b/c");
    }
}
