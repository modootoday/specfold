//! Splits a Markdown file into its frontmatter block and its body.

pub struct Split<'a> {
    /// Raw frontmatter text without the fences, if the file has one.
    pub raw: Option<&'a str>,
    pub body: &'a str,
    /// 1-based line where the body starts.
    pub body_line: usize,
}

pub fn split(text: &str) -> Split<'_> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let Some(rest) = text.strip_prefix("---\n").or_else(|| text.strip_prefix("---\r\n")) else {
        return Split { raw: None, body: text, body_line: 1 };
    };
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        let bare = line.trim_end_matches(['\n', '\r']);
        if bare == "---" || bare == "..." {
            let raw = &rest[..offset];
            return Split { raw: Some(raw), body: &rest[offset + line.len()..], body_line: raw.lines().count() + 3 };
        }
        offset += line.len();
    }
    Split { raw: None, body: text, body_line: 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_block() {
        let s = split("---\nid: x\nrel:\n  a: b\n---\n# Title\n");
        assert_eq!(s.raw, Some("id: x\nrel:\n  a: b\n"));
        assert_eq!(s.body_line, 6);
        assert!(s.body.starts_with("# Title"));
    }

    #[test]
    fn no_block() {
        assert!(split("# Just a title\n").raw.is_none());
        assert!(split("---\nunclosed\n").raw.is_none());
    }
}
