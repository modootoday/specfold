//! What a change touches inside submodules: a gitlink that moves stands for the files that
//! changed between its two commits, which only the submodule's own checkout can list.

use super::repo::{git, split_nul};
use std::path::Path;

/// Which change of the outer repository to read gitlinks from.
#[derive(Debug, Clone, Copy)]
pub enum Side<'a> {
    /// The index against HEAD.
    Staged,
    /// The working tree against a base revision.
    Since(&'a str),
}

const ZERO: &str = "0000000000000000000000000000000000000000";

/// Repository-relative paths changed inside submodules whose gitlink this change adds or moves.
/// A submodule that is not checked out, or that lacks either commit, adds nothing.
pub fn submodule_changes(root: &Path, side: Side) -> Vec<String> {
    let mut args = vec!["diff", "--raw", "-z", "--no-abbrev", "--diff-filter=AM"];
    match side {
        Side::Staged => args.push("--cached"),
        Side::Since(base) => args.push(base),
    }
    let Some(out) = git(root, &args) else { return vec![] };
    let fields = split_nul(out);
    let mut files = vec![];
    for pair in fields.chunks(2) {
        let [meta, path] = pair else { continue };
        let parts: Vec<&str> = meta.trim_start_matches(':').split(' ').collect();
        let [old_mode, new_mode, old, new, _status] = parts[..] else { continue };
        if new_mode != "160000" {
            continue;
        }
        let sub = root.join(path);
        if !sub.join(".git").exists() {
            continue;
        }
        files.extend(inner_changes(&sub, old_mode, old, new).into_iter().map(|f| format!("{path}/{f}")));
    }
    files.sort();
    files.dedup();
    files
}

/// Files changed inside one submodule between the recorded commit and the new one; a working-tree
/// side has no recorded commit, so the checkout's HEAD stands for it.
fn inner_changes(sub: &Path, old_mode: &str, old: &str, new: &str) -> Vec<String> {
    let head;
    let new = if new == ZERO {
        let Some(out) = git(sub, &["rev-parse", "HEAD"]) else { return vec![] };
        head = String::from_utf8_lossy(&out).trim().to_string();
        head.as_str()
    } else {
        new
    };
    let listed = if old_mode != "160000" || old == ZERO {
        git(sub, &["ls-tree", "-r", "-z", "--name-only", new])
    } else {
        git(sub, &["diff", "--name-only", "-z", "--diff-filter=ACMR", old, new])
    };
    listed.map(split_nul).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn run(dir: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.email=t@t", "-c", "user.name=t", "-c", "protocol.file.allow=always"])
            .args(args)
            .output()
            .unwrap()
            .status
            .success();
        assert!(ok, "git {args:?}");
    }

    #[test]
    fn a_moved_gitlink_lists_the_files_changed_inside_it() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = tmp.path().join("lib");
        std::fs::create_dir_all(&lib).unwrap();
        run(&lib, &["init", "-q"]);
        std::fs::write(lib.join("a.md"), "a").unwrap();
        run(&lib, &["add", "a.md"]);
        run(&lib, &["commit", "-qm", "a"]);
        let outer = tmp.path().join("outer");
        std::fs::create_dir_all(&outer).unwrap();
        run(&outer, &["init", "-q"]);
        run(&outer, &["submodule", "add", "-q", lib.to_str().unwrap(), "libs/lib"]);
        run(&outer, &["commit", "-qm", "pin"]);
        let sub = outer.join("libs/lib");
        std::fs::create_dir_all(sub.join(".spec/adr")).unwrap();
        std::fs::write(sub.join(".spec/adr/new.md"), "n").unwrap();
        run(&sub, &["add", ".spec/adr/new.md"]);
        run(&sub, &["commit", "-qm", "new"]);
        run(&outer, &["add", "libs/lib"]);
        assert_eq!(submodule_changes(&outer, Side::Staged), vec!["libs/lib/.spec/adr/new.md".to_string()]);
        assert_eq!(submodule_changes(&outer, Side::Since("HEAD")), vec!["libs/lib/.spec/adr/new.md".to_string()]);
    }

    #[test]
    fn an_unmoved_gitlink_adds_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        run(tmp.path(), &["init", "-q"]);
        assert!(submodule_changes(tmp.path(), Side::Staged).is_empty());
    }
}
