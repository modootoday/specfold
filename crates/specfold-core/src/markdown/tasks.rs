//! Task list items (`- [ ]` and `- [x]`), so a plan's progress can be read without parsing prose.

use serde::Serialize;

/// One checkbox item of a body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Task {
    /// 1-based line within the body.
    pub line: usize,
    pub done: bool,
    pub text: String,
}

/// Every task item outside fenced code, in order.
pub fn list(body: &str) -> Vec<Task> {
    let mut fence = crate::markdown::Fence::default();
    let mut out = Vec::new();
    for (i, l) in body.lines().enumerate() {
        let t = l.trim_start();
        if fence.step(t) {
            continue;
        }
        if fence.inside() {
            continue;
        }
        let Some(rest) = ["- ", "* ", "+ "].iter().find_map(|m| t.strip_prefix(m)) else { continue };
        let done = match rest.get(..3) {
            Some("[ ]") => false,
            Some("[x]") | Some("[X]") => true,
            _ => continue,
        };
        let text = &rest[3..];
        if !(text.is_empty() || text.starts_with(' ')) {
            continue;
        }
        out.push(Task { line: i + 1, done, text: text.trim().to_string() });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_open_and_done_items_outside_code() {
        let b = "## Steps\n- [ ] write parser\n  * [x] read spec\n- [link](x)\n```\n- [ ] not a task\n```\n+ [X] ship\n- [ ]x\n";
        let t = list(b);
        assert_eq!(t.len(), 3);
        assert_eq!(t[0], Task { line: 2, done: false, text: "write parser".into() });
        assert!(t[1].done && t[2].done);
        assert_eq!(t[2].line, 8);
    }
}
