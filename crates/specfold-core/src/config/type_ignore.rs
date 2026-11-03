//! `[types.<name>] ignore`: files in a type's directory that are neither documents nor strays,
//! such as a README or a template. Globs are relative to the type's directory.

use super::types::RawConfig;
use crate::model::registry::Registry;
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};

/// The globs as one matcher; why one of them cannot be used otherwise.
pub fn globset(globs: &[String]) -> Result<GlobSet, String> {
    let mut set = GlobSetBuilder::new();
    for g in globs {
        let t = g.trim();
        if t.is_empty() || t.starts_with(['/', '\\']) || t.split('/').any(|p| p == "..") {
            return Err(format!(
                "\"{g}\" is not a glob inside the type's directory; write it relative to it, such as \"README.md\""
            ));
        }
        let glob = GlobBuilder::new(t.trim_start_matches("./")).literal_separator(true).build();
        set.add(glob.map_err(|e| format!("\"{g}\" is not a glob: {}", e.kind()))?);
    }
    set.build().map_err(|e| e.to_string())
}

/// The first `ignore` among `raw`'s types that cannot be used as written.
pub fn problem(raw: &RawConfig) -> Option<String> {
    let registry = Registry::new(&raw.types);
    registry.types.iter().find_map(|t| globset(&t.ignore).err().map(|why| format!("[types.{}] ignore: {why}", t.name)))
}

/// Per type name, the matcher for its `ignore` globs; types without any are left out.
pub fn matchers(registry: &Registry) -> Vec<(String, GlobSet)> {
    let with = registry.types.iter().filter(|t| !t.ignore.is_empty());
    with.filter_map(|t| Some((t.name.clone(), globset(&t.ignore).ok()?))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn globs(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn ignore_globs_match_inside_the_type_dir_only() {
        let set = globset(&globs(&["README.md", "./TEMPLATE.md", "drafts/**"])).unwrap();
        assert!(set.is_match("README.md") && set.is_match("TEMPLATE.md") && set.is_match("drafts/a/b.txt"));
        assert!(!set.is_match("sub/README.md"), "a glob without a slash stays at the top");
    }

    #[test]
    fn ignore_globs_that_leave_the_type_dir_or_do_not_parse_are_refused() {
        for (g, want) in [("../x", "inside the type's directory"), ("/abs", "inside"), ("a[", "is not a glob")] {
            let got = globset(&globs(&[g])).unwrap_err();
            assert!(got.contains(want), "{g}: {got}");
        }
    }
}
