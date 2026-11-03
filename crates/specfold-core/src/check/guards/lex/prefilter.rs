//! A cheap first pass over a file. Each pattern is relaxed into one the linear-time `regex` engine
//! accepts (lookaround removed, so it matches at least everything the original does), and all of
//! them run as one set. Only patterns the set reports, on lines it reports, reach the backtracking
//! engine; most files match nothing and are read once.

use regex::{Regex, RegexSet};

/// Drops lookaround groups and `\b`, `\B`: a superset of the original, or None when it still does
/// not compile (backreferences). Without Unicode word boundaries the `regex` crate keeps its DFA
/// on non-ASCII text instead of falling back to a slower engine.
pub fn relax(pattern: &str) -> Option<String> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let mut in_class = false;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && !in_class && matches!(chars.get(i + 1), Some('b' | 'B')) {
            i += 2;
            continue;
        }
        if c == '\\' {
            out.push(c);
            if let Some(n) = chars.get(i + 1) {
                out.push(*n);
            }
            i += 2;
            continue;
        }
        if in_class {
            in_class = c != ']';
            out.push(c);
            i += 1;
            continue;
        }
        if c == '[' {
            in_class = true;
            out.push(c);
            i += 1;
            continue;
        }
        let rest: String = chars[i..chars.len().min(i + 4)].iter().collect();
        let opener = ["(?<=", "(?<!", "(?=", "(?!"].iter().find(|o| rest.starts_with(**o));
        if let Some(o) = opener {
            i = skip_group(&chars, i + o.chars().count());
            continue;
        }
        out.push(c);
        i += 1;
    }
    Regex::new(&out).ok().map(|_| out)
}

/// The index just past the `)` that closes a group whose body starts at `i`.
fn skip_group(chars: &[char], mut i: usize) -> usize {
    let (mut depth, mut in_class) = (1, false);
    while i < chars.len() && depth > 0 {
        match chars[i] {
            '\\' => i += 1,
            '[' if !in_class => in_class = true,
            ']' if in_class => in_class = false,
            '(' if !in_class => depth += 1,
            ')' if !in_class => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    i
}

/// The relaxed patterns as one set; a pattern that cannot be relaxed always runs in full.
pub struct Prefilter {
    set: RegexSet,
    /// For each pattern, its index in `set`, or None when it must always run.
    slots: Vec<Option<usize>>,
    lines: Vec<Option<Regex>>,
}

impl Prefilter {
    pub fn new(patterns: &[&str]) -> Prefilter {
        let relaxed: Vec<Option<String>> = patterns.iter().map(|p| relax(p)).collect();
        let mut members = vec![];
        let slots = relaxed
            .iter()
            .map(|r| {
                r.as_ref().map(|p| {
                    members.push(p.clone());
                    members.len() - 1
                })
            })
            .collect();
        let set = RegexSet::new(&members).unwrap_or_else(|_| RegexSet::empty());
        let lines = relaxed.iter().map(|r| r.as_ref().and_then(|p| Regex::new(p).ok())).collect();
        Prefilter { set, slots, lines }
    }

    /// Indices of the patterns worth running on `text`.
    pub fn candidates(&self, text: &str) -> Vec<usize> {
        let hit = self.set.matches(text);
        self.slots.iter().enumerate().filter(|(_, s)| s.is_none_or(|i| hit.matched(i))).map(|(p, _)| p).collect()
    }

    /// Byte ranges of the lines of `text` where pattern `p` may match; None means every line.
    pub fn lines(&self, p: usize, text: &str) -> Option<Vec<(usize, usize)>> {
        let re = self.lines.get(p)?.as_ref()?;
        let mut out: Vec<(usize, usize)> = vec![];
        for m in re.find_iter(text) {
            let start = text[..m.start()].rfind('\n').map(|i| i + 1).unwrap_or(0);
            let end = text[m.end()..].find('\n').map(|i| m.end() + i).unwrap_or(text.len());
            match out.last_mut() {
                Some(last) if start <= last.1 => last.1 = last.1.max(end),
                _ => out.push((start, end)),
            }
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relaxing_removes_lookaround_and_keeps_the_rest() {
        assert_eq!(relax(r"(?i)\bacme(?!-tools\b|\.tools\b)\b").as_deref(), Some(r"(?i)acme"));
        assert_eq!(relax(r"[\\b]x\Bq").as_deref(), Some(r"[\\b]xq"), "an escaped backslash in a class stays");
        assert_eq!(relax(r"(?<![a-z0-9_.-])/home/[a-z]+/").as_deref(), Some(r"/home/[a-z]+/"));
        assert_eq!(relax(r"(?![©®])\p{Extended_Pictographic}").as_deref(), Some(r"\p{Extended_Pictographic}"));
        assert_eq!(relax(r"[(?!]x").as_deref(), Some(r"[(?!]x"), "a class is not a group");
        assert_eq!(relax(r"(a)\1"), None, "backreferences stay with the full engine");
    }

    #[test]
    fn only_candidate_patterns_and_lines_are_kept() {
        let f = Prefilter::new(&[r"acme(?!-tools)", r"(a)\1", r"zzz"]);
        let text = "one\nacme-tools here\ntwo\n";
        assert_eq!(f.candidates(text), [0, 1]);
        let lines = f.lines(0, text).unwrap();
        assert_eq!(lines.iter().map(|(s, e)| &text[*s..*e]).collect::<Vec<_>>(), ["acme-tools here"]);
        assert!(f.lines(1, text).is_none());
    }
}
