//! When each path was first added, carried across renames, from one pass over the history and
//! cached per repository. Asking git per file with --follow costs seconds each in a long history.

use super::repo::{git, nearest_repo};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

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
