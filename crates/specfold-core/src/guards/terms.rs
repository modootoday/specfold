//! Denied terms: gathered from the guard, a file outside the repository, or an environment
//! variable, and matched case-insensitively. Terms are never printed in full.

use crate::config::{Config, GuardDef};
use anyhow::{Context, Result};
use regex::{Regex, RegexBuilder};

pub fn load(cfg: &Config, g: &GuardDef) -> Result<Vec<String>> {
    let mut terms: Vec<String> = g.deny_terms.clone();
    if let Some(f) = &g.deny_terms_file {
        let path = cfg.expand_path(f);
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("guard {}: cannot read its terms file {}", g.name, path.display()))?;
        terms.extend(text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(str::to_string));
    }
    if let Some(var) = &g.deny_terms_env
        && let Ok(v) = std::env::var(var)
    {
        terms.extend(v.split(['\n', ',']).map(str::trim).filter(|t| !t.is_empty()).map(str::to_string));
    }
    terms.sort();
    terms.dedup();
    Ok(terms)
}

/// One regex for every term, longest first so the most specific term is the one reported.
pub fn matcher(terms: &[String]) -> Option<Regex> {
    if terms.is_empty() {
        return None;
    }
    let mut sorted: Vec<&String> = terms.iter().collect();
    sorted.sort_by_key(|t| std::cmp::Reverse(t.len()));
    let alt: Vec<String> = sorted.iter().map(|t| regex::escape(t)).collect();
    RegexBuilder::new(&alt.join("|")).case_insensitive(true).build().ok()
}

/// `internal-name` → `in***me (13)`: enough to find it, not enough to leak it into a log.
pub fn mask(term: &str) -> String {
    let chars: Vec<char> = term.chars().collect();
    if chars.len() <= 4 {
        return format!("{}*** ({})", chars.first().copied().unwrap_or('*'), chars.len());
    }
    let head: String = chars[..2].iter().collect();
    let tail: String = chars[chars.len() - 2..].iter().collect();
    format!("{head}***{tail} ({})", chars.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_and_masks() {
        let re = matcher(&["acme".into(), "acme-internal".into()]).unwrap();
        assert_eq!(re.find("see ACME-Internal docs").unwrap().as_str(), "ACME-Internal");
        assert_eq!(mask("acme-internal"), "ac***al (13)");
        assert_eq!(mask("abc"), "a*** (3)");
    }
}
