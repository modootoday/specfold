//! A guard's definition turned into matchers: which paths it reads, which files may not exist,
//! and what one line trips (denied terms, patterns, secrets) after URL and allow-list exemptions.

use super::{secrets, terms};
use crate::config::{Config, GuardDef};
use crate::workspace::owners::{PathMatch, matcher};
use anyhow::{Result, anyhow};
use regex::Regex;
use std::sync::OnceLock;

pub struct Pattern {
    pub re: Regex,
    pub label: String,
    pub not_in_url: bool,
    pub exempt: Vec<PathMatch>,
    pub must_hit: Vec<String>,
    pub must_pass: Vec<String>,
}

pub struct Compiled {
    paths: Vec<PathMatch>,
    exclude: Vec<PathMatch>,
    deny_files: Vec<PathMatch>,
    terms: Option<Regex>,
    pub patterns: Vec<Pattern>,
    allow: Vec<String>,
}

pub fn compile(cfg: &Config, g: &GuardDef) -> Result<Compiled> {
    let mut patterns = vec![];
    for rule in &g.deny_regex {
        let s = rule.spec();
        let re = Regex::new(&s.re).map_err(|e| anyhow!("guard {}: bad regex {}: {e}", g.name, s.re))?;
        patterns.push(Pattern {
            label: s.label.clone().unwrap_or_else(|| s.re.clone()),
            re,
            not_in_url: s.not_in_url,
            exempt: s.exempt.iter().filter_map(|p| matcher(p)).collect(),
            must_hit: s.must_hit,
            must_pass: s.must_pass,
        });
    }
    Ok(Compiled {
        paths: g.paths.iter().filter_map(|p| matcher(p)).collect(),
        exclude: g.exclude.iter().filter_map(|p| matcher(p)).collect(),
        deny_files: g.deny_files.iter().filter_map(|p| matcher(p)).collect(),
        terms: terms::matcher(&terms::load(cfg, g)?),
        patterns,
        allow: g.allow.iter().map(|a| a.to_lowercase()).collect(),
    })
}

fn url_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"[a-zA-Z][a-zA-Z0-9+.-]*://[^\s"'<>)\]]+"#).unwrap())
}

impl Pattern {
    /// The first match on `line` that does not sit inside a URL (when asked).
    pub fn find<'a>(&self, line: &'a str) -> Option<&'a str> {
        let urls: Vec<(usize, usize)> =
            if self.not_in_url { url_re().find_iter(line).map(|m| (m.start(), m.end())).collect() } else { vec![] };
        self.re
            .find_iter(line)
            .find(|m| !urls.iter().any(|(s, e)| m.start() >= *s && m.end() <= *e))
            .map(|m| m.as_str())
    }
}

impl Compiled {
    fn allowed(&self, text: &str) -> bool {
        let t = text.to_lowercase();
        self.allow.iter().any(|a| t.contains(a.as_str()))
    }

    /// What one line of `path` trips, as messages; terms are masked.
    pub fn line_hits(&self, g: &GuardDef, path: &str, line: &str) -> Vec<String> {
        let mut out = vec![];
        if let Some(m) = self.terms.as_ref().and_then(|t| t.find(line))
            && !self.allowed(m.as_str())
        {
            out.push(format!("denied term {}", terms::mask(m.as_str())));
        }
        for p in self.patterns.iter().filter(|p| !p.exempt.iter().any(|m| m.is_match(path))) {
            if let Some(m) = p.find(line)
                && !self.allowed(m)
            {
                out.push(format!("matches {}", p.label));
            }
        }
        if g.secrets {
            for s in secrets::all() {
                if s.re.find(line).is_some_and(|m| !self.allowed(m.as_str())) {
                    out.push(format!("looks like a {}", s.name));
                }
            }
        }
        out
    }

    pub fn in_scope(&self, path: &str) -> bool {
        (self.paths.is_empty() || self.paths.iter().any(|m| m.is_match(path)))
            && !self.exclude.iter().any(|m| m.is_match(path))
    }

    pub fn denies_file(&self, path: &str) -> bool {
        self.deny_files.iter().any(|m| m.is_match(path))
    }
}
