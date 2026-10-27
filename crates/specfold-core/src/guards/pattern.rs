//! One denied pattern and where its matches do not count: inside a URL, inside a regex literal
//! of a script, inside code in documentation, or in an exempt path. Patterns may use lookaround,
//! which project rules written for other engines rely on.

use super::{code_spans, jslex};
use crate::config::RegexSpec;
use crate::workspace::owners::{PathMatch, matcher};
use anyhow::{Result, anyhow};
use fancy_regex::Regex;

pub struct Pattern {
    pub re: Regex,
    pub label: String,
    not_in_url: bool,
    not_in_regex_literal: bool,
    code_exempt: Vec<PathMatch>,
    exempt: Vec<PathMatch>,
    pub must_hit: Vec<String>,
    pub must_pass: Vec<String>,
}

/// Spans of a file's text computed once, on demand, for every pattern that asks.
#[derive(Default)]
pub struct Spans {
    regex_literals: Option<Vec<(usize, usize)>>,
    code: Option<Vec<(usize, usize)>>,
}

fn within(spans: &[(usize, usize)], at: usize) -> bool {
    spans.iter().any(|(s, e)| at >= *s && at < *e)
}

/// True when the match sits inside a URL: the token around it, bounded by whitespace, quotes
/// and brackets, has a scheme before it.
fn inside_url(text: &str, at: usize) -> bool {
    let start = text[..at].rfind(|c: char| c.is_whitespace() || "\"'`(<".contains(c)).map(|i| i + 1).unwrap_or(0);
    text[start..at].contains("://")
}

pub fn compile(guard: &str, s: RegexSpec) -> Result<Pattern> {
    let re = Regex::new(&s.re).map_err(|e| anyhow!("guard {guard}: bad regex {}: {e}", s.re))?;
    Ok(Pattern {
        label: s.label.clone().unwrap_or_else(|| s.re.clone()),
        re,
        not_in_url: s.not_in_url,
        not_in_regex_literal: s.not_in_regex_literal,
        code_exempt: s.code_exempt.iter().filter_map(|p| matcher(p)).collect(),
        exempt: s.exempt.iter().filter_map(|p| matcher(p)).collect(),
        must_hit: s.must_hit,
        must_pass: s.must_pass,
    })
}

impl Pattern {
    pub fn applies_to(&self, path: &str) -> bool {
        !self.exempt.iter().any(|m| m.is_match(path))
    }

    /// Matches in `text` (offset and matched text) that count for `path`.
    pub fn hits<'t>(&self, path: &str, text: &'t str, spans: &mut Spans) -> Vec<(usize, &'t str)> {
        self.hits_in(path, text, &[(0, text.len())], spans)
    }

    /// Like `hits`, searching only the byte ranges given (the lines a prefilter kept); offsets
    /// stay relative to the whole text so exemptions see the full context.
    pub fn hits_in<'t>(
        &self,
        path: &str,
        text: &'t str,
        ranges: &[(usize, usize)],
        spans: &mut Spans,
    ) -> Vec<(usize, &'t str)> {
        let mut out = vec![];
        let found = ranges
            .iter()
            .flat_map(|&(s, e)| self.re.find_iter(&text[s..e]).flatten().map(move |m| (s + m.start(), s + m.end())));
        for (at, end) in found {
            if self.not_in_url && inside_url(text, at) {
                continue;
            }
            if self.not_in_regex_literal && jslex::is_script(path) {
                let lits = spans.regex_literals.get_or_insert_with(|| jslex::regex_literal_spans(text));
                if within(lits, at) {
                    continue;
                }
            }
            if self.code_exempt.iter().any(|c| c.is_match(path)) {
                let code = spans.code.get_or_insert_with(|| code_spans::spans(text));
                if within(code, at) {
                    continue;
                }
            }
            out.push((at, &text[at..end]));
        }
        out
    }

    /// Whether a single fixture line trips this pattern, judged as an ordinary file.
    pub fn trips(&self, line: &str) -> bool {
        !self.hits("<fixture>", line, &mut Spans::default()).is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pat(toml_spec: &str) -> Pattern {
        compile("g", toml::from_str(toml_spec).unwrap()).unwrap()
    }

    #[test]
    fn lookaround_url_and_regex_literal_exemptions() {
        let brand = pat("re = '(?i)\\bacme(?!-tools\\b)\\b'");
        assert!(brand.trips("the acme worker") && !brand.trips("acme-tools plugin"));
        let host = pat("re = '(?<![a-zA-Z0-9_.-])/srv/[a-z]+/'\nnot_in_url = true");
        assert!(host.trips("/srv/build/x") && !host.trips("see https://x.com/docs/srv/acct/keys"));
        let greek = pat("re = '[α-ω]'\nnot_in_regex_literal = true");
        let src = "const rule = /ωμ/;\nconst said = \"ωμ\";\n";
        assert_eq!(greek.hits("a.mjs", src, &mut Spans::default()).len(), 2, "only the string's two letters");
        assert_eq!(greek.hits("a.md", src, &mut Spans::default()).len(), 4);
    }

    #[test]
    fn code_in_listed_docs_is_an_example() {
        let emoji = pat("re = '\\p{Extended_Pictographic}'\ncode_exempt = [\"docs/skills/**\"]");
        let text = "Shows `\u{1F600}` here.\nAnd \u{1F600} in prose.\n";
        assert_eq!(emoji.hits("docs/skills/a/SKILL.md", text, &mut Spans::default()).len(), 1);
        assert_eq!(emoji.hits("README.md", text, &mut Spans::default()).len(), 2);
    }
}
