//! `<!-- spec:count ... -->N<!-- /spec:count -->` markers: a number in prose that names where
//! its true value comes from, so a check can compare and a fix can rewrite it.

const OPEN: &str = "<!-- spec:count";
const CLOSE: &str = "<!-- /spec:count -->";

/// One well-formed marker on a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marker {
    /// 1-based line in the scanned text.
    pub line: usize,
    /// The whole marker as written, for an exact replacement.
    pub raw: String,
    /// The words between `spec:count` and `-->`.
    pub source: String,
    /// What the marker currently wraps, trimmed.
    pub value: String,
}

/// A marker that cannot be read; the message says why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Malformed {
    pub line: usize,
    pub message: String,
}

/// Blanks inline code spans so markers quoted in backticks are not read; byte offsets are kept.
fn mask_code(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut inside = false;
    for c in line.chars() {
        if c == '`' {
            inside = !inside;
            out.push(c);
        } else {
            if inside {
                out.extend(std::iter::repeat_n(' ', c.len_utf8()));
            } else {
                out.push(c);
            }
        }
    }
    out
}

fn scan_line(n: usize, line: &str, ok: &mut Vec<Marker>, bad: &mut Vec<Malformed>) {
    let masked = mask_code(line);
    let mut rest = 0;
    while rest < masked.len() {
        let hay = &masked[rest..];
        let (Some(o), c) = (hay.find(OPEN), hay.find(CLOSE)) else {
            if hay.contains(CLOSE) {
                bad.push(Malformed {
                    line: n,
                    message: "closing <!-- /spec:count --> without an opening marker".into(),
                });
            }
            return;
        };
        if c.is_some_and(|c| c < o) {
            bad.push(Malformed { line: n, message: "closing <!-- /spec:count --> without an opening marker".into() });
        }
        let start = rest + o;
        let Some(head_end) = masked[start..].find("-->").map(|i| start + i) else {
            bad.push(Malformed { line: n, message: "spec:count marker is not closed with -->".into() });
            return;
        };
        let source = masked[start + OPEN.len()..head_end].trim().to_string();
        let body_start = head_end + 3;
        let Some(end) = masked[body_start..].find(CLOSE).map(|i| body_start + i) else {
            bad.push(Malformed { line: n, message: "spec:count has no <!-- /spec:count --> on the same line".into() });
            return;
        };
        let value = &masked[body_start..end];
        if value.contains(OPEN) {
            bad.push(Malformed { line: n, message: "spec:count markers cannot be nested".into() });
            return;
        }
        let stop = end + CLOSE.len();
        ok.push(Marker { line: n, raw: line[start..stop].to_string(), source, value: value.trim().to_string() });
        rest = stop;
    }
}

/// Every marker in a Markdown text, skipping fenced code; malformed markers are returned apart.
pub fn scan(text: &str) -> (Vec<Marker>, Vec<Malformed>) {
    let (mut ok, mut bad) = (vec![], vec![]);
    let mut fence: Option<String> = None;
    for (i, line) in text.lines().enumerate() {
        let t = line.trim_start();
        let ch = t.chars().next().unwrap_or(' ');
        if ch == '`' || ch == '~' {
            let run: String = t.chars().take_while(|c| *c == ch).collect();
            if run.len() >= 3 {
                match &fence {
                    Some(f) if run.starts_with(f.as_str()) && t[run.len()..].trim().is_empty() => fence = None,
                    None => fence = Some(run),
                    _ => {}
                }
                continue;
            }
        }
        if fence.is_none() {
            scan_line(i + 1, line, &mut ok, &mut bad);
        }
    }
    (ok, bad)
}

/// The marker with its value replaced.
pub fn rewrite(m: &Marker, n: usize) -> String {
    let head_end = m.raw.find("-->").map(|i| i + 3).unwrap_or(0);
    format!("{}{n}{CLOSE}", &m.raw[..head_end])
}

/// Body rows of the pipe tables in `section` of `text`: header and separator lines not counted.
pub fn table_rows(text: &str, section: &str) -> Option<usize> {
    let (lo, hi) = super::sections::span(text, section)?;
    let lines: Vec<&str> = text.lines().collect();
    let mut rows = 0;
    let mut in_table = false;
    for l in &lines[lo..hi] {
        let Some(cells) = super::tables::cells(l) else {
            in_table = false;
            continue;
        };
        let sep = cells.iter().all(|c| !c.is_empty() && c.chars().all(|ch| matches!(ch, '-' | ':' | ' ')));
        if !in_table {
            in_table = true;
        } else if !sep {
            rows += 1;
        }
    }
    Some(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_markers_and_skips_code() {
        let t = "a <!-- spec:count find type=adr -->3<!-- /spec:count --> b\n```\n<!-- spec:count x -->1<!-- /spec:count -->\n```\n`<!-- spec:count x -->1<!-- /spec:count -->`\n";
        let (ok, bad) = scan(t);
        assert_eq!(ok.len(), 1);
        assert_eq!(ok[0].source, "find type=adr");
        assert_eq!(ok[0].value, "3");
        assert_eq!(rewrite(&ok[0], 5), "<!-- spec:count find type=adr -->5<!-- /spec:count -->");
        assert!(bad.is_empty());
    }

    #[test]
    fn reports_malformed() {
        for t in [
            "<!-- spec:count find -->3",
            "3<!-- /spec:count -->",
            "<!-- spec:count find --><!-- spec:count find -->1<!-- /spec:count -->",
        ] {
            assert_eq!(scan(t).1.len(), 1, "{t}");
        }
    }

    #[test]
    fn counts_table_rows() {
        let t = "# A\n## Items\n| k | v |\n|---|---|\n| a | 1 |\n| b | 2 |\n## B\n| x |\n|---|\n| y |\n";
        assert_eq!(table_rows(t, "Items"), Some(2));
        assert_eq!(table_rows(t, "Nope"), None);
    }
}
