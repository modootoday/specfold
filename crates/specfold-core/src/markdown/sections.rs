//! Extracting one section of a document, so a reader (or an agent) pays only for what it reads.

/// The heading line whose text or anchor matches `name` (case-insensitive), and everything below
/// it up to the next heading of the same or higher level.
pub fn extract(body: &str, name: &str) -> Option<String> {
    let want = name.trim().trim_start_matches('#').to_lowercase();
    let lines: Vec<&str> = body.lines().collect();
    let level = |l: &str| -> Option<usize> {
        let t = l.trim_start();
        let n = t.chars().take_while(|c| *c == '#').count();
        (n > 0 && t[n..].starts_with(' ')).then_some(n)
    };
    let start = lines.iter().position(|l| {
        level(l).is_some() && {
            let text = l.trim_start().trim_start_matches('#').trim().to_lowercase();
            text == want || text.starts_with(&format!("{want} ")) || text.contains(&format!("{{#{want}}}"))
        }
    })?;
    let lv = level(lines[start])?;
    let end = lines[start + 1..]
        .iter()
        .position(|l| level(l).is_some_and(|n| n <= lv))
        .map(|i| start + 1 + i)
        .unwrap_or(lines.len());
    Some(lines[start..end].join("\n"))
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
    }
}
