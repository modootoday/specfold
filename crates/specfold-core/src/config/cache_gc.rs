//! Keeps the per-user cache directory bounded (B4). Every cache file is per repository path, so
//! throwaway clones, worktrees and test repositories leave files nobody reads again. A file is
//! touched when it is read, so its modification time is its last use; pruning removes files
//! unused for 30 days, then the least recently used ones while the directory holds more than
//! 64 MB. It runs at most once an hour and ignores every failure: the cache is an optimisation.

use std::path::Path;
use std::time::{Duration, SystemTime};

const MAX_AGE: Duration = Duration::from_secs(30 * 24 * 3600);
const MAX_BYTES: u64 = 64 * 1024 * 1024;
const EVERY: Duration = Duration::from_secs(3600);
/// A temporary file older than this was left by a run that died before its rename.
const TMP_AGE: Duration = Duration::from_secs(3600);
const STAMP: &str = ".last-prune";

fn age(t: SystemTime, now: SystemTime) -> Duration {
    now.duration_since(t).unwrap_or_default()
}

/// Marks a cache file as just used, at most once a day so warm runs do not write.
pub fn touch(file: &Path) {
    let now = SystemTime::now();
    let old = std::fs::metadata(file).and_then(|m| m.modified()).is_ok_and(|t| age(t, now) > EVERY * 24);
    if old && let Ok(f) = std::fs::File::options().append(true).open(file) {
        let _ = f.set_modified(now);
    }
}

/// Prunes `dir` when the last prune is over an hour old; returns how many files it removed.
pub fn prune(dir: &Path) -> usize {
    let now = SystemTime::now();
    let stamp = dir.join(STAMP);
    if std::fs::metadata(&stamp).and_then(|m| m.modified()).is_ok_and(|t| age(t, now) < EVERY) {
        return 0;
    }
    if !dir.is_dir() || std::fs::write(&stamp, b"").is_err() {
        return 0;
    }
    let Ok(read) = std::fs::read_dir(dir) else { return 0 };
    let mut files: Vec<(SystemTime, u64, std::path::PathBuf)> = read
        .flatten()
        .filter(|e| e.file_name() != STAMP)
        .filter_map(|e| {
            let m = e.metadata().ok().filter(|m| m.is_file())?;
            Some((m.modified().ok()?, m.len(), e.path()))
        })
        .collect();
    let mut removed = 0;
    files.retain(|(t, _, p)| {
        let tmp = p.extension().is_some_and(|x| x.to_string_lossy().starts_with("tmp"));
        let drop = age(*t, now) > MAX_AGE || (tmp && age(*t, now) > TMP_AGE);
        if drop && std::fs::remove_file(p).is_ok() {
            removed += 1;
        }
        !drop
    });
    files.sort_by_key(|(t, _, _)| *t);
    let mut total: u64 = files.iter().map(|(_, n, _)| n).sum();
    for (_, n, p) in &files {
        if total <= MAX_BYTES {
            break;
        }
        if std::fs::remove_file(p).is_ok() {
            total -= n;
            removed += 1;
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn b4_prune_drops_old_and_stale_tmp_files_once_an_hour() {
        let d = tempfile::tempdir().expect("tempdir");
        let old = SystemTime::now() - MAX_AGE - Duration::from_secs(60);
        for (name, t) in [("scan-old.json", old), ("scan-new.json", SystemTime::now()), ("scan-x.tmp9", old)] {
            std::fs::write(d.path().join(name), b"{}").expect("write");
            std::fs::File::options()
                .append(true)
                .open(d.path().join(name))
                .expect("open")
                .set_modified(t)
                .expect("mtime");
        }
        assert_eq!(prune(d.path()), 2);
        assert!(d.path().join("scan-new.json").exists());
        std::fs::write(d.path().join("scan-old2.json"), b"{}").expect("write");
        std::fs::File::options()
            .append(true)
            .open(d.path().join("scan-old2.json"))
            .expect("open")
            .set_modified(old)
            .expect("mtime");
        assert_eq!(prune(d.path()), 0, "a second prune within the hour does nothing");
    }
}
