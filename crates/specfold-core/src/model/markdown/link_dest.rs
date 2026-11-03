//! Link destinations in a body, the way the import and `mv` rewrite them (C180): inline links
//! and images (`[t](a.md#x)`, `[t](<a b.md>)`) and reference definitions (`[r]: a.md "title"`),
//! outside fenced and inline code. A percent-encoded destination is matched decoded and written
//! back encoded.

use crate::export::links::{md_link_re, md_ref_def_re, outside_code};

/// Byte ranges of every link destination in `body` (without `<` `>`), in order.
pub fn spans(body: &str) -> Vec<(usize, usize)> {
    let mut out = vec![];
    for (re, groups) in [(md_link_re(), [3, 4]), (md_ref_def_re(), [2, 3])] {
        outside_code(body, re, |start, m| {
            if let Some(g) = groups.iter().find_map(|&i| m.get(i)) {
                out.push((start + g.start(), start + g.end()));
            }
        });
    }
    out.sort_unstable();
    out
}

/// `a%20b.md` → `a b.md`; text that does not decode to UTF-8 stays as written.
pub fn decode(p: &str) -> String {
    let b = p.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |c: u8| (c as char).to_digit(16);
        match (b[i], b.get(i + 1).copied().and_then(hex), b.get(i + 2).copied().and_then(hex)) {
            (b'%', Some(h), Some(l)) => {
                out.push((h * 16 + l) as u8);
                i += 3;
            }
            (c, _, _) => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| p.to_string())
}

/// Percent-encodes everything but unreserved characters and `/`.
pub fn encode(p: &str) -> String {
    p.bytes()
        .map(|c| match c {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => (c as char).to_string(),
            _ => format!("%{c:02X}"),
        })
        .collect()
}

/// Rewrites each destination whose path (decoded, `#fragment` and `?query` apart) `new_path`
/// maps to a new path; the fragment is kept, and an encoded destination stays encoded. Returns
/// the body and how many destinations changed.
pub fn rewrite(body: &str, mut new_path: impl FnMut(&str) -> Option<String>) -> (String, usize) {
    let mut out = String::with_capacity(body.len());
    let (mut at, mut n) = (0, 0);
    for (a, b) in spans(body) {
        let dest = &body[a..b];
        let cut = dest.find(['#', '?']).unwrap_or(dest.len());
        let (path, tail) = dest.split_at(cut);
        if path.is_empty() {
            continue;
        }
        let Some(new) = new_path(&decode(path)) else { continue };
        let new = if path.contains('%') { encode(&new) } else { new };
        out.push_str(&body[at..a]);
        out.push_str(&new);
        out.push_str(tail);
        at = b;
        n += 1;
    }
    out.push_str(&body[at..]);
    (out, n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_link_form_is_rewritten_outside_code() {
        let body = "[a](old.md#x) [b](<old.md>) [c](%EC%98%9B.md)\n\n[r]: old.md \"t\"\n`[d](old.md)`\n```\n[e](old.md)\n```\n";
        let (out, n) = rewrite(body, |p| matches!(p, "old.md" | "옛.md").then(|| "new dir/새.md".to_string()));
        assert_eq!(n, 4);
        assert_eq!(
            out,
            "[a](new dir/새.md#x) [b](<new dir/새.md>) [c](new%20dir/%EC%83%88.md)\n\n[r]: new dir/새.md \"t\"\n`[d](old.md)`\n```\n[e](old.md)\n```\n"
        );
        assert_eq!(decode("%ZZ%41"), "%ZZA");
    }
}
