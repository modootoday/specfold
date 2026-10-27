//! Expands the map's patterns into import units: one main file with its chapters and
//! attachments.

use super::map::{ImportMap, SourceRule};
use crate::workspace::owners::{PathMatch, matcher};
use ignore::WalkBuilder;
use regex::Regex;
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Unit {
    pub rule: usize,
    /// The file or directory that matched.
    pub source: String,
    pub main: String,
    pub chapters: Vec<String>,
    pub attachments: Vec<String>,
}

const MAIN_NAMES: &[&str] = &["index.md", "README.md", "readme.md", "00-index.md", "_index.md"];

fn all_files(root: &Path) -> Vec<String> {
    WalkBuilder::new(root)
        .hidden(false)
        .require_git(false)
        .filter_entry(|e| e.file_name() != ".git")
        .build()
        .flatten()
        .filter(|e| e.file_type().is_some_and(|t| t.is_file()))
        .filter_map(|e| e.path().strip_prefix(root).ok().map(|p| p.to_string_lossy().replace('\\', "/")))
        .collect()
}

/// The main file, chapters and attachments of a directory, leaving out files an earlier rule
/// already took (a nested bundle belongs to the rule that matched it first).
fn inside<'a>(files: &'a [String], dir: &str, claimed: &BTreeSet<String>) -> Vec<&'a String> {
    let prefix = format!("{dir}/");
    files.iter().filter(|f| f.starts_with(&prefix) && !claimed.contains(*f)).collect()
}

fn bundle(files: &[String], dir: &str, claimed: &BTreeSet<String>) -> Option<(String, Vec<String>, Vec<String>)> {
    let inside = inside(files, dir, claimed);
    let main = MAIN_NAMES
        .iter()
        .map(|n| format!("{dir}/{n}"))
        .find(|m| inside.contains(&m))
        .or_else(|| inside.iter().filter(|f| f.ends_with(".md")).min().map(|s| s.to_string()))?;
    let chapters = inside.iter().filter(|f| f.ends_with(".md") && **f != &main).map(|s| s.to_string()).collect();
    let attachments = inside.iter().filter(|f| !f.ends_with(".md")).map(|s| s.to_string()).collect();
    Some((main, chapters, attachments))
}

pub fn expand(root: &Path, map: &ImportMap) -> Vec<Unit> {
    let files = all_files(root);
    let mut claimed: BTreeSet<String> = BTreeSet::new();
    let mut out = vec![];
    for (ri, rule) in map.sources.iter().enumerate() {
        out.extend(expand_rule(root, &files, ri, rule, &mut claimed));
    }
    out
}

/// A redirect rule claims only the files whose text is a stub, so a broad glob over a directory
/// that mixes stubs and real documents leaves the documents to the rules after it.
fn is_stub(root: &Path, file: &str, re: Option<&Regex>) -> bool {
    let Some(re) = re else { return true };
    std::fs::read_to_string(root.join(file)).is_ok_and(|t| re.is_match(&t))
}

fn expand_rule(
    root: &Path,
    files: &[String],
    ri: usize,
    rule: &SourceRule,
    claimed: &mut BTreeSet<String>,
) -> Vec<Unit> {
    let dir_mode = rule.glob.ends_with('/');
    let Some(m) = matcher(rule.glob.trim_end_matches('/')) else { return vec![] };
    let exclude: Vec<PathMatch> = rule.exclude.iter().filter_map(|e| matcher(e)).collect();
    let skip = |p: &str, claimed: &BTreeSet<String>| exclude.iter().any(|e| e.is_match(p)) || claimed.contains(p);
    let mut out = vec![];
    if dir_mode {
        let mut dirs: BTreeSet<String> = BTreeSet::new();
        for f in files {
            let mut parts: Vec<&str> = f.split('/').collect();
            parts.pop();
            for n in 1..=parts.len() {
                let d = parts[..n].join("/");
                if m.is_match(&d) {
                    dirs.insert(d);
                }
            }
        }
        for d in dirs {
            if skip(&d, claimed) {
                continue;
            }
            let unit = match bundle(files, &d, claimed) {
                Some((main, chapters, attachments)) => Some((main, chapters, attachments)),
                None if rule.wrap || rule.drop => {
                    let all: Vec<String> = inside(files, &d, claimed).into_iter().cloned().collect();
                    (!all.is_empty()).then(|| (String::new(), vec![], all))
                }
                None => None,
            };
            if let Some((main, chapters, attachments)) = unit {
                for f in std::iter::once(&main).chain(&chapters).chain(&attachments) {
                    claimed.insert(f.clone());
                }
                claimed.insert(d.clone());
                out.push(Unit { rule: ri, source: d, main, chapters, attachments });
            }
        }
        return out;
    }
    let stub_re = rule.redirect.as_deref().and_then(|r| Regex::new(r).ok());
    for f in files {
        if !m.is_match(f) || skip(f, claimed) {
            continue;
        }
        if rule.redirect.is_some() && !is_stub(root, f, stub_re.as_ref()) {
            continue;
        }
        claimed.insert(f.clone());
        if rule.wrap && !f.ends_with(".md") {
            out.push(Unit {
                rule: ri,
                source: f.clone(),
                main: String::new(),
                chapters: vec![],
                attachments: vec![f.clone()],
            });
            continue;
        }
        let (mut chapters, mut attachments) = (vec![], vec![]);
        if rule.sibling_dir {
            let dir = f.trim_end_matches(".md");
            let taken: Vec<String> = inside(files, dir, claimed).into_iter().cloned().collect();
            for g in taken {
                if g.ends_with(".md") {
                    chapters.push(g.clone())
                } else {
                    attachments.push(g.clone())
                }
                claimed.insert(g);
            }
        }
        out.push(Unit { rule: ri, source: f.clone(), main: f.clone(), chapters, attachments });
    }
    out
}
