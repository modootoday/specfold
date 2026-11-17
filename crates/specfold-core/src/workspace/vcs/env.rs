//! The repository-local git variables a call inherits: a hook exports the committing
//! repository's GIT_DIR and GIT_INDEX_FILE, which must not leak into a call on another one.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, PoisonError};

/// What `git rev-parse --local-env-vars` lists: each names one repository, not every repository.
const LOCAL: &[&str] = &[
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_CONFIG",
    "GIT_CONFIG_COUNT",
    "GIT_CONFIG_PARAMETERS",
    "GIT_DIR",
    "GIT_GRAFT_FILE",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_INTERNAL_SUPER_PREFIX",
    "GIT_NO_REPLACE_OBJECTS",
    "GIT_OBJECT_DIRECTORY",
    "GIT_PREFIX",
    "GIT_REPLACE_REF_BASE",
    "GIT_SHALLOW_FILE",
    "GIT_WORK_TREE",
];

/// Which repository a call reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Reach {
    /// The repository a run is rooted in: it keeps the index a hook names when that hook runs
    /// for this repository (commit --only stages into a temporary index).
    Root,
    /// A submodule or nested checkout: nothing of the outer repository's environment applies.
    Inner,
}

/// `git -C dir` with the repository-local environment cleared, the hook's index kept for `Root`.
pub(super) fn command(dir: &Path, reach: Reach) -> Command {
    let mut c = Command::new("git");
    c.arg("-C").arg(dir);
    clear(&mut c);
    if reach == Reach::Root
        && let Some(index) = hook_index(dir)
    {
        c.env("GIT_INDEX_FILE", index);
    }
    c
}

fn clear(c: &mut Command) {
    for v in LOCAL {
        c.env_remove(v);
    }
    for (k, _) in std::env::vars_os() {
        let k = k.to_string_lossy();
        if k.starts_with("GIT_CONFIG_KEY_") || k.starts_with("GIT_CONFIG_VALUE_") {
            c.env_remove(k.as_ref());
        }
    }
}

static INDEX: Mutex<Option<HashMap<PathBuf, Option<OsString>>>> = Mutex::new(None);

/// GIT_INDEX_FILE made absolute, when it belongs to the repository `dir` is in: GIT_DIR, or
/// without it the repository of the directory we were started in, is that repository.
fn hook_index(dir: &Path) -> Option<OsString> {
    let index = std::env::var_os("GIT_INDEX_FILE")?;
    let mut cache = INDEX.lock().unwrap_or_else(PoisonError::into_inner);
    let map = cache.get_or_insert_with(HashMap::new);
    map.entry(dir.to_path_buf()).or_insert_with(|| owned_index(dir, index)).clone()
}

fn owned_index(dir: &Path, index: OsString) -> Option<OsString> {
    let index = absolute(PathBuf::from(index));
    let hook_dir = match std::env::var_os("GIT_DIR") {
        Some(d) => absolute(PathBuf::from(d)),
        None => git_dir(&std::env::current_dir().ok()?)?,
    };
    let own = git_dir(dir)?;
    let same = |a: &Path, b: &Path| a.canonicalize().ok().zip(b.canonicalize().ok()).is_some_and(|(a, b)| a == b);
    same(&own, &hook_dir).then(|| index.into_os_string())
}

/// The git directory `dir` is in, found without the inherited environment.
fn git_dir(dir: &Path) -> Option<PathBuf> {
    let mut c = Command::new("git");
    c.arg("-C").arg(dir).args(["rev-parse", "--absolute-git-dir"]);
    clear(&mut c);
    let out = c.output().ok().filter(|o| o.status.success())?;
    Some(PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()))
}

/// A path the hook gave relative to the directory git started it in, which is ours.
fn absolute(p: PathBuf) -> PathBuf {
    if p.is_absolute() {
        return p;
    }
    std::env::current_dir().map(|d| d.join(&p)).unwrap_or(p)
}
