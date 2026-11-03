//! Slash-separated paths cleaned up as text: `.` and `..` resolved without the file system.

/// `a/./b/../c` as `a/c`; `None` when it climbs above where it starts.
pub fn normalize(path: &str) -> Option<String> {
    let mut out: Vec<&str> = vec![];
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                out.pop()?;
            }
            p => out.push(p),
        }
    }
    Some(out.join("/"))
}

/// A link read from the folder `dir`: `docs/adr` and `../x/./y.md` give `docs/x/y.md`; `None`
/// when it climbs above the root. Only the link is cleaned up; `dir` is taken as given.
pub fn join(dir: &str, link: &str) -> Option<String> {
    let mut parts: Vec<&str> = dir.split('/').filter(|s| !s.is_empty()).collect();
    for seg in link.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    Some(parts.join("/"))
}

/// Like [`normalize`], for keys that must stay comparable when the path leaves its start: a `..`
/// with nothing left to remove is kept, and `\` separates like `/`.
pub fn normalize_keeping_up(path: &str) -> String {
    let mut parts: Vec<&str> = vec![];
    for c in path.split(['/', '\\']) {
        match c {
            "" | "." => {}
            ".." if parts.last().is_some_and(|l| *l != "..") => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_resolves_dots_and_refuses_climbing_out() {
        let cases: &[(&str, Option<&str>)] = &[
            ("a/b/../c", Some("a/c")),
            ("a/./b/../c", Some("a/c")),
            ("./a//b/", Some("a/b")),
            ("a/..", Some("")),
            ("", Some("")),
            ("../x", None),
            ("a/../../x", None),
        ];
        for (input, want) in cases {
            assert_eq!(normalize(input).as_deref(), *want, "{input}");
        }
    }

    #[test]
    fn join_reads_the_link_from_the_folder() {
        let cases: &[(&str, &str, Option<&str>)] = &[
            ("doc/adr", "../x/./y.md", Some("doc/x/y.md")),
            ("", "../y.md", None),
            ("", "a.md", Some("a.md")),
            ("docs/", "./a.md", Some("docs/a.md")),
            ("a/b", "../../c.md", Some("c.md")),
            ("a/b", "../../../c.md", None),
        ];
        for (dir, link, want) in cases {
            assert_eq!(join(dir, link).as_deref(), *want, "{dir} + {link}");
        }
    }

    #[test]
    fn normalize_keeping_up_keeps_leading_parents() {
        let cases: &[(&str, &str)] = &[
            ("./b.json", "b.json"),
            ("x/../b.json", "b.json"),
            ("../b.json", "../b.json"),
            ("../../a/../b", "../../b"),
            ("a\\b\\..\\c", "a/c"),
            ("", ""),
        ];
        for (input, want) in cases {
            assert_eq!(normalize_keeping_up(input), *want, "{input}");
        }
    }
}
