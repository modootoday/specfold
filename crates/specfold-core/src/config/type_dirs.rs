//! `[types.<name>] dir`: a type that keeps its documents in a directory of its own, relative to
//! each owner's root. Each such directory stays inside the owner and apart from the spec
//! directory, the local directory and every other type's directory, so a path has one reading.

use super::types::RawConfig;
use crate::model::registry::Registry;
use crate::paths::is_under;

/// The directory `dir` names, cleaned up; why it cannot be a type directory otherwise.
pub fn normalize(dir: &str) -> Result<String, String> {
    let d = dir.trim();
    let drive = d.len() > 1 && d.as_bytes()[1] == b':';
    if d.starts_with(['/', '\\', '~']) || drive {
        return Err("is absolute; name a directory inside the owner, such as \"adr\"".into());
    }
    if d.split(['/', '\\']).any(|p| p == "..") {
        return Err("climbs out of the owner with '..'; name a directory inside the owner, such as \"adr\"".into());
    }
    if d.contains('\\') {
        return Err("uses '\\'; separate directories with '/'".into());
    }
    match crate::paths::normalize(d) {
        Some(n) if !n.is_empty() => Ok(n),
        _ => Err("names the owner's root itself; name a directory inside it, such as \"adr\"".into()),
    }
}

/// The first `dir` among `raw`'s types that cannot be used as written.
pub fn problem(raw: &RawConfig) -> Option<String> {
    let registry = Registry::new(&raw.types);
    let spec = crate::paths::normalize(&raw.spec.dir).unwrap_or_default();
    let local = crate::paths::normalize(&raw.local.path).unwrap_or_default();
    let mut seen: Vec<(&str, String)> = vec![];
    for t in &registry.types {
        let Some(dir) = &t.dir else { continue };
        let at = format!("[types.{}] dir = \"{dir}\"", t.name);
        let d = match normalize(dir) {
            Ok(d) => d,
            Err(why) => return Some(format!("{at} {why}")),
        };
        if is_under(&d, &spec) || is_under(&spec, &d) {
            return Some(format!(
                "{at} overlaps [spec] dir \"{spec}\", where documents are found by their type's subdirectory; pick a directory outside it, or drop dir to keep {spec}/{}",
                t.name
            ));
        }
        if is_under(&d, &local) || is_under(&local, &d) {
            return Some(format!("{at} overlaps [local] path \"{local}\"; pick a directory outside it"));
        }
        for (other, od) in &seen {
            if d == *od {
                return Some(format!(
                    "[types.{other}] and [types.{}] both have dir = \"{d}\"; give each type its own directory",
                    t.name
                ));
            }
            if is_under(&d, od) || is_under(od, &d) {
                return Some(format!(
                    "{at} and [types.{other}] dir = \"{od}\" nest; a type's directory may not hold another's"
                ));
            }
        }
        seen.push((&t.name, d));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::registry::TypeOverride;

    fn with_dirs(dirs: &[(&str, &str)]) -> RawConfig {
        let mut raw = RawConfig::default();
        for (ty, dir) in dirs {
            let o = TypeOverride { dir: Some(dir.to_string()), about: Some("x".into()), ..Default::default() };
            raw.types.insert(ty.to_string(), o);
        }
        raw
    }

    #[test]
    fn type_dirs_inside_the_owner_and_apart_pass() {
        assert_eq!(problem(&with_dirs(&[("adr", "adr"), ("proposal", "./proposals/")])), None);
        assert_eq!(normalize("./docs//adr/").as_deref(), Ok("docs/adr"));
        assert_eq!(problem(&RawConfig::default()), None);
    }

    #[test]
    fn type_dir_errors_name_the_conflict() {
        let cases: &[(&[(&str, &str)], &str)] = &[
            (&[("adr", "decisions"), ("proposal", "decisions")], "both have dir"),
            (&[("adr", "docs"), ("proposal", "docs/proposals")], "nest"),
            (&[("adr", "../adr")], "climbs out"),
            (&[("adr", "/srv/adr")], "is absolute"),
            (&[("adr", "C:/adr")], "is absolute"),
            (&[("adr", ".")], "owner's root"),
            (&[("adr", ".spec/decisions")], "overlaps [spec] dir"),
            (&[("adr", ".spec")], "overlaps [spec] dir"),
            (&[("adr", ".spec.local/adr")], "overlaps [local] path"),
        ];
        for (dirs, want) in cases {
            let got = problem(&with_dirs(dirs)).unwrap_or_default();
            assert!(got.contains(want), "{dirs:?}: {got}");
        }
    }
}
