//! Built-in secret patterns. Deliberately narrow: each one is a format with a fixed prefix or
//! header, so a hit is almost always real.

use regex::Regex;
use std::sync::OnceLock;

pub struct SecretPattern {
    pub name: &'static str,
    pub re: Regex,
}

const PATTERNS: &[(&str, &str)] = &[
    ("private key", r"-----BEGIN (?:RSA |EC |DSA |OPENSSH |PGP |ENCRYPTED )?PRIVATE KEY(?: BLOCK)?-----"),
    ("AWS access key id", r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b"),
    ("GitHub token", r"\b(?:gh[pousr]_[A-Za-z0-9]{36,}|github_pat_[A-Za-z0-9_]{40,})\b"),
    ("GitLab token", r"\bglpat-[A-Za-z0-9_-]{20,}\b"),
    ("Slack token", r"\bxox[abposr]-[A-Za-z0-9-]{10,}\b"),
    ("Google API key", r"\bAIza[0-9A-Za-z_-]{35}\b"),
    ("Stripe live key", r"\b(?:sk|rk)_live_[0-9A-Za-z]{20,}\b"),
    ("npm token", r"\bnpm_[A-Za-z0-9]{36}\b"),
    ("Anthropic API key", r"\bsk-ant-[A-Za-z0-9_-]{20,}\b"),
    ("OpenAI API key", r"\bsk-(?:proj|svcacct|admin)-[A-Za-z0-9_-]{20,}\b"),
    ("Hugging Face token", r"\bhf_[A-Za-z]{34}\b"),
];

pub fn all() -> &'static [SecretPattern] {
    static P: OnceLock<Vec<SecretPattern>> = OnceLock::new();
    P.get_or_init(|| {
        PATTERNS.iter().filter_map(|(name, re)| Regex::new(re).ok().map(|re| SecretPattern { name, re })).collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Samples are assembled at run time so this file never holds a string that looks like a
    // real credential to secret scanners.
    #[test]
    fn detects_known_formats_only() {
        let hit = |s: &str| all().iter().find(|p| p.re.is_match(s)).map(|p| p.name);
        assert_eq!(hit(&format!("-----BEGIN OPENSSH {}-----", "PRIVATE KEY")), Some("private key"));
        assert_eq!(hit(&format!("key {}{} here", "AK", "IA".to_string() + &"X".repeat(16))), Some("AWS access key id"));
        assert_eq!(hit(&format!("{}_{}", "ghp", "a".repeat(36))), Some("GitHub token"));
        assert_eq!(hit("the word secret and sk_test_123"), None);
    }
}
