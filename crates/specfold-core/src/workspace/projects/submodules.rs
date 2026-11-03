//! The submodules of the repository, read from one place: the gitlinks the index records first,
//! then any `.gitmodules` entry the index does not hold (a checkout without git, a pending add).

use crate::config::defaults::CONFIG_FILE;
use std::path::Path;

/// Repository-relative paths of every submodule, gitlinks first.
pub fn submodules(root: &Path) -> Vec<String> {
    let mut out = crate::workspace::vcs::gitlinks(root);
    for (p, _) in declared(root) {
        if !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

/// Every submodule with the `url` `.gitmodules` gives it, in the order of `submodules`; a gitlink
/// that `.gitmodules` does not declare has none.
pub fn submodule_urls(root: &Path) -> Vec<(String, Option<String>)> {
    let declared = declared(root);
    submodules(root)
        .into_iter()
        .map(|path| {
            let url = declared.iter().find(|(p, _)| *p == path).and_then(|(_, u)| u.clone());
            (path, url)
        })
        .collect()
}

/// A1: the submodules that carry their own spec.toml, as (remote name, path); the last path
/// segment names each one.
pub fn submodule_remotes(root: &Path) -> Vec<(String, String)> {
    submodules(root)
        .into_iter()
        .filter_map(|path| {
            let name = path.rsplit('/').next().unwrap_or(&path).to_string();
            (!name.is_empty() && root.join(&path).join(CONFIG_FILE).is_file()).then_some((name, path))
        })
        .collect()
}

/// The `path` of each `.gitmodules` section, with the `url` of the same section.
fn declared(root: &Path) -> Vec<(String, Option<String>)> {
    let Ok(text) = std::fs::read_to_string(root.join(".gitmodules")) else { return vec![] };
    let mut out: Vec<(String, Option<String>)> = vec![];
    let (mut path, mut url): (Option<String>, Option<String>) = (None, None);
    let mut flush = |path: &mut Option<String>, url: &mut Option<String>| {
        if let Some(p) = path.take().filter(|p| !p.is_empty()) {
            out.push((p, url.take()));
        }
        *url = None;
    };
    for line in text.lines() {
        if line.trim_start().starts_with('[') {
            flush(&mut path, &mut url);
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        let v = v.trim().trim_matches('"');
        match k.trim() {
            "path" => path = Some(v.trim_end_matches('/').to_string()),
            "url" => url = Some(v.to_string()),
            _ => {}
        }
    }
    flush(&mut path, &mut url);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declared_pairs_each_path_with_its_section_url() {
        let dir = tempfile::tempdir().unwrap();
        let text = "[submodule \"a\"]\n\turl = https://example.com/a.git\n\tpath = libs/a/\n[submodule \"b\"]\n\tpath = \"libs/b\"\n";
        std::fs::write(dir.path().join(".gitmodules"), text).unwrap();
        let got = declared(dir.path());
        assert_eq!(
            got,
            vec![("libs/a".to_string(), Some("https://example.com/a.git".to_string())), ("libs/b".to_string(), None)]
        );
    }
}
