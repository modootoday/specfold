//! Whether a slash-separated path lies in a folder, judged by whole components: `ab/c` is not
//! in `a`.

/// `path` is `base` itself or lies below it.
pub fn is_under(path: &str, base: &str) -> bool {
    path == base || is_below(path, base)
}

/// `path` lies below `base`, `base` itself excluded.
pub fn is_below(path: &str, base: &str) -> bool {
    path.strip_prefix(base).is_some_and(|rest| rest.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn under_and_below_compare_whole_components() {
        let cases: &[(&str, &str, bool, bool)] = &[
            ("a/b", "a", true, true),
            ("a", "a", true, false),
            ("ab/c", "a", false, false),
            ("a/b/c", "a/b", true, true),
            ("b", "a", false, false),
            ("a/", "a", true, true),
            ("", "", true, false),
            ("/x", "", true, true),
            ("x", "", false, false),
        ];
        for (path, base, under, below) in cases {
            assert_eq!(is_under(path, base), *under, "{path} under {base}");
            assert_eq!(is_below(path, base), *below, "{path} below {base}");
        }
    }
}
