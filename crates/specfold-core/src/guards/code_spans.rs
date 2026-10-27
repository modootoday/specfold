//! Code in Markdown and HTML: fenced blocks, inline code and <code> elements. Text there is an
//! example a document shows, not prose it says.

use regex::Regex;
use std::sync::OnceLock;

fn code_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?s)```.*?```|`[^`\n]+`|<code(?:\s[^>]*)?>.*?</code>").unwrap())
}

/// Byte spans `[start, end)` of code in `text`.
pub fn spans(text: &str) -> Vec<(usize, usize)> {
    code_re().find_iter(text).map(|m| (m.start(), m.end())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_fences_inline_and_elements() {
        let t = "prose\n```\nfence\n```\nsay `inline` and <code class=\"x\">el</code> end";
        let s = spans(t);
        let covered = |w: &str| {
            let i = t.find(w).unwrap();
            s.iter().any(|(a, b)| i >= *a && i < *b)
        };
        assert!(covered("fence") && covered("inline") && covered("el<"));
        assert!(!covered("prose") && !covered("end"));
    }
}
