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
    let (lines, levels) = heading_levels(body);
    let start = match matches(body, name).as_slice() {
        [one] => *one,
        _ => return None,
    };
    let lv = levels[start]?;
    let end = (start + 1..lines.len()).find(|&i| levels[i].is_some_and(|n| n <= lv)).unwrap_or(lines.len());
    Some((start, end))
}

fn heading_levels(body: &str) -> (Vec<&str>, Vec<Option<usize>>) {
    let lines: Vec<&str> = body.lines().collect();
    let mut fence = crate::model::markdown::Fence::default();
    let levels = lines
        .iter()
        .map(|l| {
            if fence.step(l) || fence.inside() {
                return None;
            }
            crate::model::markdown::atx(l).map(|(n, _)| n)
        })
        .collect();
    (lines, levels)
}

/// C198: the heading lines `name` picks. An exact heading text or `{#anchor}` wins over a heading
/// that only starts with the name; a prefix match counts only when there is no exact one. More
/// than one result means the name is ambiguous, and [`span`] refuses it.
pub fn matches(body: &str, name: &str) -> Vec<usize> {
    let want = name.trim().trim_start_matches('#').to_lowercase();
    let (lines, levels) = heading_levels(body);
    let text = |i: usize| head_text(lines[i]).to_lowercase();
    let heads: Vec<usize> = (0..lines.len()).filter(|&i| levels[i].is_some()).collect();
    let exact: Vec<usize> =
        heads.iter().copied().filter(|&i| text(i) == want || text(i).contains(&format!("{{#{want}}}"))).collect();
    if !exact.is_empty() {
        return exact;
    }
    heads.into_iter().filter(|&i| text(i).starts_with(&format!("{want} "))).collect()
}

/// The error for a name [`span`] does not resolve: several candidates, or none. `first` is the
/// file line of the body's first line, so the lines named are file lines (C226).
pub fn not_found(body: &str, first: usize, name: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let hits = matches(body, name);
    if hits.len() > 1 {
        let list: Vec<String> = hits.iter().map(|&i| format!("line {} '{}'", i + first, head_text(lines[i]))).collect();
        return format!(
            "section '{name}' matches {} headings ({}); name one exactly, or give it an {{#anchor}}",
            hits.len(),
            list.join(" | ")
        );
    }
    format!("no section '{name}' (sections: {})", names(body).join(" | "))
}

/// The section names an error lists when a name is not found: every heading but the H1 title.
pub fn names(body: &str) -> Vec<String> {
    outline(body).into_iter().filter(|(n, _)| *n > 1).map(|(_, t)| t).collect()
}

/// Heading outline: `(level, text)` for every heading.
pub fn outline(body: &str) -> Vec<(usize, String)> {
    let mut fence = crate::model::markdown::Fence::default();
    body.lines()
        .filter_map(|l| {
            if fence.step(l) || fence.inside() {
                return None;
            }
            crate::model::markdown::atx(l).map(|(n, t)| (n, t.to_string()))
        })
        .collect()
}

/// The text of a heading line (already known to be one).
fn head_text(line: &str) -> &str {
    crate::model::markdown::atx(line).map_or("", |(_, t)| t)
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
        assert_eq!(names(b), ["Rule", "Detail {#R-1}", "Why"], "the H1 title is not a section");
        assert_eq!(span(b, "why"), Some((5, 7)));
        assert_eq!(span("## A\n```\n## not\n```\n## B\n", "a"), Some((0, 4)));
    }

    #[test]
    fn c198_exact_beats_prefix_and_ambiguity_refuses() {
        let b = "## Steps to roll back\nr\n## Steps\ns\n";
        assert_eq!(span(b, "steps"), Some((2, 4)));
        assert_eq!(span(b, "steps to"), Some((0, 2)), "one prefix match still resolves");
        let p = "## Steps one\n## Steps two\n";
        assert_eq!(span(p, "steps"), None);
        assert!(not_found(p, 1, "steps").contains("matches 2 headings"), "{}", not_found(p, 1, "steps"));
        let d = "## Notes\na\n## Notes\nb\n";
        assert_eq!(span(d, "notes"), None, "duplicate titles are ambiguous");
        assert!(not_found(d, 5, "notes").contains("line 7 'Notes'"));
        assert!(not_found(d, 1, "zz").starts_with("no section"));
    }

    #[test]
    fn c238_indented_code_and_seven_hashes_are_not_headings() {
        let b = "## Decision\nRun this:\n\n    # install the tool\n    cargo install x\n\n####### seven\nThen use Postgres.\n## Next\n";
        assert_eq!(outline(b), [(2, "Decision".to_string()), (2, "Next".to_string())]);
        assert_eq!(span(b, "decision"), Some((0, 8)));
        assert_eq!(span(b, "seven"), None);
        assert_eq!(outline("   ### three spaces\n#\n"), [(3, "three spaces".to_string()), (1, String::new())]);
    }

    #[test]
    fn c240_closing_hashes_are_not_heading_text() {
        let b = "## Context ##\n## Decision #\n## C# ##\n## Tag#\n### ###\n";
        let t: Vec<String> = outline(b).into_iter().map(|(_, t)| t).collect();
        assert_eq!(t, ["Context", "Decision", "C#", "Tag#", ""]);
        assert_eq!(span(b, "context"), Some((0, 1)));
    }
}
