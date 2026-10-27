//! Guards: named checks over any file in the repository and over commit messages, such as the
//! gate that keeps internal names out of a public package.

pub mod range;
mod scan;
pub mod secrets;
pub mod terms;

use crate::check::finding::Finding;
use crate::config::Config;
use anyhow::{Result, bail};
pub use range::GuardRange;

/// Runs the named guards (all of them when `names` is empty). A guard without an explicit range
/// uses its own default.
pub fn run(cfg: &Config, names: &[String], range: Option<&GuardRange>) -> Result<Vec<Finding>> {
    let mut selected = vec![];
    for n in names {
        match cfg.raw.guards.iter().find(|g| &g.name == n) {
            Some(g) => selected.push(g),
            None => bail!(
                "no guard named '{n}' (have: {})",
                cfg.raw.guards.iter().map(|g| g.name.as_str()).collect::<Vec<_>>().join(", ")
            ),
        }
    }
    if names.is_empty() {
        selected = cfg.raw.guards.iter().collect();
    }
    let mut out = vec![];
    for g in selected {
        let r = range.cloned().unwrap_or_else(|| GuardRange::parse(&g.range));
        let resolved = range::resolve(&cfg.root, &r)?;
        out.extend(scan::run(cfg, g, &resolved)?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{GuardDef, RawConfig};

    #[test]
    fn scans_tree() {
        let tmp = tempfile::tempdir().unwrap();
        let r = tmp.path();
        std::fs::create_dir_all(r.join("libs/open/src")).unwrap();
        std::fs::create_dir_all(r.join("services/shop")).unwrap();
        let fake_key = format!("{}{}", "AK", "IA".to_string() + &"X".repeat(16));
        std::fs::write(r.join("libs/open/src/a.rs"), format!("// built for Acme-Internal\nlet k = \"{fake_key}\";\n"))
            .unwrap();
        std::fs::write(r.join("libs/open/.env"), "X=1\n").unwrap();
        std::fs::write(r.join("services/shop/b.rs"), "acme-internal is fine here\n").unwrap();
        std::fs::write(r.join("terms.txt"), "# private\nacme-internal\n").unwrap();
        let raw = RawConfig {
            guards: vec![GuardDef {
                name: "public".into(),
                about: String::new(),
                paths: vec!["libs/open/**".into()],
                exclude: vec![],
                range: "all".into(),
                deny_terms: vec![],
                deny_terms_file: Some("terms.txt".into()),
                deny_terms_env: None,
                deny_regex: vec![],
                deny_files: vec!["**/.env*".into()],
                max_file_bytes: None,
                secrets: true,
                commit_messages: false,
                level: "error".into(),
            }],
            ..Default::default()
        };
        let cfg = Config::from_raw(r.to_path_buf(), raw, false);
        let mut f = run(&cfg, &[], None).unwrap();
        f.sort_by_key(|a| (a.path.clone(), a.line));
        let got: Vec<(String, usize)> = f.iter().map(|x| (x.path.clone(), x.line)).collect();
        assert_eq!(
            got,
            [
                ("libs/open/.env".to_string(), 1),
                ("libs/open/src/a.rs".to_string(), 1),
                ("libs/open/src/a.rs".to_string(), 2)
            ]
        );
        assert!(!f[1].message.contains("acme-internal"));
    }
}
