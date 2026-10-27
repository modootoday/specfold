//! Regular-expression literal spans in JavaScript and TypeScript source, found by a small lexer
//! that knows strings, template literals, comments and when a slash starts a regex. A rule that
//! recognises some wording has to spell it, and that spelling is not a leaked sentence.

const SCRIPT_EXTENSIONS: &[&str] = &["js", "mjs", "cjs", "jsx", "ts", "mts", "cts", "tsx"];
/// Characters after which a slash starts a regex rather than a division.
const REGEX_AFTER: &str = "(,=:[!&|?{};+-*%<>~^";
/// Keywords after which a slash starts a regex.
const REGEX_AFTER_WORDS: &[&str] =
    &["return", "typeof", "case", "in", "of", "delete", "void", "throw", "new", "else", "do", "yield", "await"];

pub fn is_script(path: &str) -> bool {
    path.rsplit_once('.').is_some_and(|(_, ext)| SCRIPT_EXTENSIONS.contains(&ext))
}

fn skip_until(chars: &[(usize, char)], mut i: usize, end: char, stop_at_newline: bool) -> usize {
    while i < chars.len() {
        let c = chars[i].1;
        if c == '\\' {
            i += 2;
            continue;
        }
        if c == end || (stop_at_newline && c == '\n') {
            return i + 1;
        }
        i += 1;
    }
    i
}

/// The end (exclusive index into `chars`) of a regex literal starting at `start`, or None when
/// the slash turns out to be a division.
fn regex_end(chars: &[(usize, char)], start: usize) -> Option<usize> {
    let mut i = start + 1;
    let mut in_class = false;
    while i < chars.len() {
        match chars[i].1 {
            '\\' => i += 1,
            '\n' => return None,
            '[' => in_class = true,
            ']' => in_class = false,
            '/' if !in_class => {
                i += 1;
                while i < chars.len() && chars[i].1.is_ascii_alphabetic() {
                    i += 1;
                }
                return Some(i);
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Byte spans `[start, end)` of every regex literal in `text`.
pub fn regex_literal_spans(text: &str) -> Vec<(usize, usize)> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let byte_at = |i: usize| chars.get(i).map(|c| c.0).unwrap_or(text.len());
    let mut spans = vec![];
    let (mut prev, mut word) = (None::<char>, String::new());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i].1;
        let next = chars.get(i + 1).map(|x| x.1);
        if c == '/' && next == Some('/') {
            i = skip_until(&chars, i + 2, '\n', false);
            continue;
        }
        if c == '/' && next == Some('*') {
            let mut j = i + 2;
            while j + 1 < chars.len() && !(chars[j].1 == '*' && chars[j + 1].1 == '/') {
                j += 1;
            }
            i = j + 2;
            continue;
        }
        if c == '"' || c == '\'' || c == '`' {
            i = skip_until(&chars, i + 1, c, c != '`');
            prev = Some('x');
            word.clear();
            continue;
        }
        if c == '/' {
            let starts_regex =
                prev.is_none_or(|p| REGEX_AFTER.contains(p)) || REGEX_AFTER_WORDS.contains(&word.as_str());
            if starts_regex && let Some(end) = regex_end(&chars, i) {
                spans.push((byte_at(i), byte_at(end)));
                i = end;
                prev = Some('x');
                word.clear();
                continue;
            }
        }
        if c.is_alphanumeric() || c == '_' || c == '$' {
            if !prev.is_some_and(|p| p.is_alphanumeric() || p == '_' || p == '$') {
                word.clear();
            }
            word.push(c);
            prev = Some(c);
        } else if !c.is_whitespace() {
            prev = Some(c);
            word.clear();
        }
        i += 1;
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inside(text: &str, needle_from: usize) -> Vec<bool> {
        let spans = regex_literal_spans(text);
        text.match_indices('금')
            .filter(|(i, _)| *i >= needle_from)
            .map(|(i, _)| spans.iter().any(|(s, e)| i >= *s && i < *e))
            .collect()
    }

    #[test]
    fn a_regex_literal_is_told_from_strings_comments_and_division() {
        let probe = "const rule = /금지/;\nconst said = \"금지\"; // 금지\n";
        assert_eq!(inside(probe, 0), [true, false, false]);
        assert!(regex_literal_spans("const x = a / b / c;").is_empty());
        assert_eq!(regex_literal_spans("x = y / 2; z = /re[/]x/g;").len(), 1);
        assert_eq!(regex_literal_spans("if (ok) return /a\\/b/.test(s);").len(), 1);
        assert!(regex_literal_spans("/* not /a/ */ const t = `no /a/ here`;").is_empty());
    }

    #[test]
    fn script_files() {
        assert!(is_script("src/a.mts") && is_script("b.jsx"));
        assert!(!is_script("README.md"));
    }
}
