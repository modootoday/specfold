//! Extracting one section of a document, so a reader (or an agent) pays only for what it reads.

/// The heading line whose text or anchor matches `name` (case-insensitive), and everything below
/// it up to the next heading of the same or higher level.
pub fn extract(body: &str, name: &str) -> Option<String> {
    let lines: Vec<&str> = body.lines().collect();
    span(body, name).map(|(start, end)| lines[start..end].join("\n"))
}

/// Line range `[heading, end)` of the section `extract` would return, so writers can splice it.
/// Headings inside fenced code are not headings.
pub fn span(body: &str, name: &str) -> Option<(usize, usize)> {
    let want = name.trim().trim_start_matches('#').to_lowercase();
    let lines: Vec<&str> = body.lines().collect();
    let mut fence = false;
    let levels: Vec<Option<usize>> = lines
        .iter()
        .map(|l| {
            let t = l.trim_start();
            if t.starts_with("```") || t.starts_with("~~~") {
                fence = !fence;
                return None;
            }
            let n = t.chars().take_while(|c| *c == '#').count();
            (!fence && n > 0 && t[n..].starts_with(' ')).then_some(n)
        })
        .collect();
    let start = (0..lines.len()).find(|&i| {
        levels[i].is_some() && {
            let text = lines[i].trim_start().trim_start_matches('#').trim().to_lowercase();
            text == want || text.starts_with(&format!("{want} ")) || text.contains(&format!("{{#{want}}}"))
        }
    })?;
    let lv = levels[start]?;
    let end = (start + 1..lines.len()).find(|&i| levels[i].is_some_and(|n| n <= lv)).unwrap_or(lines.len());
    Some((start, end))
}

/// Heading outline: `(level, text)` for every heading.
pub fn outline(body: &str) -> Vec<(usize, String)> {
    let mut fence = false;
    body.lines()
        .filter_map(|l| {
            let t = l.trim_start();
            if t.starts_with("```") || t.starts_with("~~~") {
                fence = !fence;
            }
            if fence {
                return None;
            }
            let n = t.chars().take_while(|c| *c == '#').count();
            (n > 0 && t[n..].starts_with(' ')).then(|| (n, t[n..].trim().to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_by_heading_or_anchor() {
        let b = "# T\n## Rule\nr1\n### Detail {#R-1}\nd\n## Why\nw\n";
        assert_eq!(extract(b, "rule").unwrap(), "## Rule\nr1\n### Detail {#R-1}\nd");
        assert_eq!(extract(b, "R-1").unwrap(), "### Detail {#R-1}\nd");
        assert!(extract(b, "nope").is_none());
        assert_eq!(outline(b).len(), 4);
        assert_eq!(span(b, "why"), Some((5, 7)));
        assert_eq!(span("## A\n```\n## not\n```\n## B\n", "a"), Some((0, 4)));
    }
}
