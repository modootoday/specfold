//! A guard's definition turned into matchers: which paths it reads (its own and, when asked,
//! every published path), which files and paths may not exist, and what a text trips (denied
//! terms, patterns, secrets) after allow-list exemptions.

use super::pattern::{self, Pattern, Spans};
use super::{secrets, terms};
use crate::config::{Config, GuardDef};
use crate::workspace::exposure;
use crate::workspace::owners::{PathMatch, matcher};
use anyhow::Result;
use regex::Regex;

pub struct Compiled {
    /// Whether `paths` limits the scope; without it the guard reads the whole repository.
    limited: bool,
    paths: Vec<PathMatch>,
    exclude: Vec<PathMatch>,
    deny_files: Vec<PathMatch>,
    terms: Option<Regex>,
    pub patterns: Vec<Pattern>,
    pub path_patterns: Vec<Pattern>,
    allow: Vec<String>,
    /// Published paths that do not exist.
    pub missing: Vec<String>,
}

pub fn compile(cfg: &Config, g: &GuardDef) -> Result<Compiled> {
    let compile_all = |rules: &[crate::config::RegexRule]| -> Result<Vec<Pattern>> {
        rules.iter().map(|r| pattern::compile(&g.name, r.spec())).collect()
    };
    let mut paths: Vec<PathMatch> = g.paths.iter().filter_map(|p| matcher(p)).collect();
    let mut missing = vec![];
    if g.public {
        for e in exposure::scan(cfg).exposed {
            if !cfg.root.join(&e.path).exists() {
                missing.push(e.path.clone());
            }
            paths.extend(matcher(&e.path));
        }
    }
    Ok(Compiled {
        limited: !g.paths.is_empty() || g.public,
        paths,
        exclude: g.exclude.iter().filter_map(|p| matcher(p)).collect(),
        deny_files: g.deny_files.iter().filter_map(|p| matcher(p)).collect(),
        terms: terms::matcher(&terms::load(cfg, g)?),
        patterns: compile_all(&g.deny_regex)?,
        path_patterns: compile_all(&g.deny_path_regex)?,
        allow: g.allow.iter().map(|a| a.to_lowercase()).collect(),
        missing,
    })
}

fn line_of(newlines: &[usize], at: usize) -> usize {
    newlines.partition_point(|&n| n < at) + 1
}

impl Compiled {
    fn allowed(&self, text: &str) -> bool {
        let t = text.to_lowercase();
        self.allow.iter().any(|a| t.contains(a.as_str()))
    }

    /// What `text` of `path` trips, as (line, message); one entry per line and kind of hit.
    /// Terms are masked.
    pub fn text_hits(&self, g: &GuardDef, path: &str, text: &str) -> Vec<(usize, String)> {
        let newlines: Vec<usize> = text.match_indices('\n').map(|(i, _)| i).collect();
        let mut out: Vec<(usize, String)> = vec![];
        if let Some(t) = &self.terms {
            for m in t.find_iter(text).filter(|m| !self.allowed(m.as_str())) {
                out.push((line_of(&newlines, m.start()), format!("denied term {}", terms::mask(m.as_str()))));
            }
        }
        let mut spans = Spans::default();
        for p in self.patterns.iter().filter(|p| p.applies_to(path)) {
            for (at, hit) in p.hits(path, text, &mut spans) {
                if !self.allowed(hit) {
                    out.push((line_of(&newlines, at), format!("matches {}", p.label)));
                }
            }
        }
        if g.secrets {
            for s in secrets::all() {
                for m in s.re.find_iter(text).filter(|m| !self.allowed(m.as_str())) {
                    out.push((line_of(&newlines, m.start()), format!("looks like a {}", s.name)));
                }
            }
        }
        out.sort();
        out.dedup();
        out
    }

    /// Path patterns the repository-relative `path` itself trips.
    pub fn path_hits(&self, path: &str) -> Vec<String> {
        self.path_patterns
            .iter()
            .filter(|p| p.applies_to(path) && p.re.is_match(path).unwrap_or(false))
            .map(|p| format!("path matches {}", p.label))
            .collect()
    }

    pub fn in_scope(&self, path: &str) -> bool {
        (!self.limited || self.paths.iter().any(|m| m.is_match(path))) && !self.exclude.iter().any(|m| m.is_match(path))
    }

    pub fn denies_file(&self, path: &str) -> bool {
        self.deny_files.iter().any(|m| m.is_match(path))
    }
}
