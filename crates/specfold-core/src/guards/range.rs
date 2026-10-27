//! What a guard looks at: the index, the commits about to be pushed, or the whole tree.

use crate::workspace::vcs;
use anyhow::{Result, anyhow};
use ignore::WalkBuilder;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardRange {
    Staged,
    /// A git range such as `origin/main..HEAD`; `None` means upstream..HEAD.
    Push(Option<String>),
    All,
}

impl GuardRange {
    pub fn parse(s: &str) -> Self {
        match s {
            "staged" => GuardRange::Staged,
            "all" => GuardRange::All,
            "push" => GuardRange::Push(None),
            other => GuardRange::Push(Some(other.to_string())),
        }
    }
}

/// Where file contents are read from for a range.
#[derive(Debug, Clone)]
pub enum Source {
    Index,
    Rev(String),
    WorkTree,
}

pub struct Resolved {
    pub files: Vec<String>,
    pub source: Source,
    /// The git range for commit scanning, when there is one.
    pub commits: Option<String>,
}

pub fn resolve(root: &Path, range: &GuardRange) -> Result<Resolved> {
    match range {
        GuardRange::Staged => Ok(Resolved {
            files: vcs::staged(root).ok_or_else(|| anyhow!("--staged needs a git repository"))?,
            source: Source::Index,
            commits: None,
        }),
        GuardRange::Push(r) => {
            let r = match r {
                Some(r) => r.clone(),
                None => format!(
                    "{}..HEAD",
                    vcs::upstream(root).ok_or_else(|| anyhow!("no upstream branch; pass --range <base>..HEAD"))?
                ),
            };
            let files = vcs::changed_in_range(root, &r).ok_or_else(|| anyhow!("cannot read the range {r}"))?;
            Ok(Resolved { files, source: Source::Rev("HEAD".into()), commits: Some(r) })
        }
        GuardRange::All => {
            let files = WalkBuilder::new(root)
                .hidden(false)
                .require_git(false)
                .filter_entry(|e| e.file_name() != ".git")
                .build()
                .flatten()
                .filter(|e| e.file_type().is_some_and(|t| t.is_file()))
                .filter_map(|e| e.path().strip_prefix(root).ok().map(|p| p.to_string_lossy().replace('\\', "/")))
                .collect();
            Ok(Resolved { files, source: Source::WorkTree, commits: None })
        }
    }
}

pub fn read(root: &Path, source: &Source, path: &str) -> Option<Vec<u8>> {
    match source {
        Source::Index => vcs::show_staged(root, path).map(String::into_bytes),
        Source::Rev(rev) => vcs::show(root, rev, path).map(String::into_bytes),
        Source::WorkTree => std::fs::read(root.join(path)).ok(),
    }
}
