//! C231: file versions a range's commits carry that its end no longer has. Each of them is
//! pushed with the range and stays in history, so a guard over the range must read them too.

use super::repo::git;
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

/// One version of a file some commit in the range wrote.
pub struct Passing {
    pub commit: String,
    pub path: String,
    pub bytes: Vec<u8>,
}

/// The revision a range ends at: the part after `..` or `...`, HEAD when it is empty or the
/// range is a single revision.
pub fn range_end(range: &str) -> String {
    let end = range.rsplit_once("...").or_else(|| range.rsplit_once("..")).map(|(_, e)| e).unwrap_or("");
    if end.is_empty() { "HEAD".into() } else { end.to_string() }
}

/// (commit, path, blob) for every file a commit in `range` added or changed, newest first.
fn written(root: &Path, range: &str) -> Vec<(String, String, String)> {
    let args = [
        "-c",
        "core.quotePath=false",
        "log",
        "--format=%H",
        "--raw",
        "--no-abbrev",
        "--no-renames",
        "--diff-filter=ACM",
        range,
    ];
    let Some(out) = git(root, &args) else { return vec![] };
    let mut commit = String::new();
    let mut rows = vec![];
    for line in String::from_utf8_lossy(&out).lines() {
        if let Some(raw) = line.strip_prefix(':') {
            let Some((meta, path)) = raw.split_once('\t') else { continue };
            if let Some(blob) = meta.split_whitespace().nth(3) {
                rows.push((commit.clone(), path.to_string(), blob.to_string()));
            }
        } else if !line.is_empty() {
            commit = line.trim().to_string();
        }
    }
    rows
}

/// Blob ids in the tree at `rev`.
fn end_blobs(root: &Path, rev: &str) -> HashSet<String> {
    let Some(out) = git(root, &["ls-tree", "-r", rev]) else { return HashSet::new() };
    String::from_utf8_lossy(&out).lines().filter_map(|l| l.split_whitespace().nth(2).map(str::to_string)).collect()
}

/// Reads blobs through one `git cat-file --batch`, so a long range costs one process.
fn cat(root: &Path, ids: &[String]) -> HashMap<String, Vec<u8>> {
    let mut map = HashMap::new();
    let child = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let Ok(mut child) = child else { return map };
    let (Some(mut stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else { return map };
    let input: String = ids.iter().map(|i| format!("{i}\n")).collect();
    let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
    let mut r = BufReader::new(stdout);
    for id in ids {
        let mut head = String::new();
        if r.read_line(&mut head).unwrap_or(0) == 0 {
            break;
        }
        let Some(size) = head.split_whitespace().nth(2).and_then(|s| s.parse::<usize>().ok()) else { continue };
        let mut buf = vec![0; size + 1];
        if r.read_exact(&mut buf).is_err() {
            break;
        }
        buf.pop();
        map.insert(id.clone(), buf);
    }
    let _ = writer.join();
    let _ = child.wait();
    map
}

/// Versions written inside `range` that differ from every blob at its end: a value one commit
/// adds and a later one removes is still pushed. Each distinct blob is read once.
pub fn passing(root: &Path, range: &str) -> Vec<Passing> {
    let rows = written(root, range);
    if rows.is_empty() {
        return vec![];
    }
    let at_end = end_blobs(root, &range_end(range));
    let mut seen = HashSet::new();
    let rows: Vec<_> = rows.into_iter().filter(|(_, _, b)| !at_end.contains(b) && seen.insert(b.clone())).collect();
    let ids: Vec<String> = rows.iter().map(|(_, _, b)| b.clone()).collect();
    let mut blobs = cat(root, &ids);
    rows.into_iter().filter_map(|(commit, path, b)| Some(Passing { commit, path, bytes: blobs.remove(&b)? })).collect()
}
