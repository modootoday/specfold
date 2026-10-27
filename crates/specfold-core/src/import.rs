//! Importing documents from another layout through a project's import map.

pub mod convert;
pub mod key;
pub mod map;
pub mod owner;
pub mod plan;
pub mod sources;

pub use map::ImportMap;
pub use plan::{ImportPlan, Row, alias_table, build};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, OwnerKind, RawConfig};
    use crate::corpus::{Corpus, LoadOptions};

    fn write(root: &std::path::Path, rel: &str, text: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    #[test]
    fn imports_docs_bundles_and_stubs() {
        let tmp = tempfile::tempdir().unwrap();
        let r = tmp.path();
        std::fs::create_dir_all(r.join("services/shop/src")).unwrap();
        write(
            r,
            "docs/adr/0007-use-postgres.md",
            "---\nstatus: accepted\ndate: 2026-01-05\n---\n# Use Postgres\nTouches services/shop/src/db.rs and services/shop/src/x.rs\n",
        );
        write(r, "docs/plans/2026-02-01-launch/README.md", "# Launch\n");
        write(r, "docs/plans/2026-02-01-launch/10-steps.md", "# Steps\n");
        write(r, "docs/plans/2026-02-01-launch/board.png", "png");
        write(r, "old/launch.md", "> Moved to `docs/plans/2026-02-01-launch`\n");
        let map: ImportMap = toml::from_str(
            r#"
[status]
accepted = "active"
[[sources]]
glob = "docs/adr/*.md"
type = "adr"
key = "created"
name = '^(?P<alias>\d{4})-(?P<slug>.+)\.md$'
[[sources]]
glob = "docs/plans/*/"
type = "research"
key = "name"
name = '^(?P<key>\d{4}-\d{2}-\d{2})-(?P<slug>.+)$'
default_status = "archived"
[[sources]]
glob = "old/*.md"
redirect = 'Moved to `(?P<target>[^`]+)`'
"#,
        )
        .unwrap();
        let mut raw = RawConfig::default();
        raw.owners.kinds = vec![OwnerKind { pattern: "services/*".into(), id: "{1}".into(), manifest: false }];
        let c = Corpus::load(Config::from_raw(r.to_path_buf(), raw, false), &LoadOptions::default());
        let p = build(&c, &map).unwrap();
        let ids: Vec<&str> = p.rows.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["adr:shop/20260105000000", "research:root/20260201000000", "research:root/20260201000000"]);
        assert_eq!(p.rows[0].status_to, "active");
        assert_eq!(p.rows[0].aliases, ["0007"]);
        assert_eq!(p.rows[1].path, ".spec/research/20260201000000-launch/index.research.md");
        assert!(p.alias_rows.contains(&("old/launch.md".to_string(), "research:root/20260201000000".to_string())));
        p.plan.apply(r).unwrap();
        let c2 = Corpus::load(Config::from_raw(r.to_path_buf(), c.cfg.raw.clone(), false), &LoadOptions::default());
        assert_eq!(c2.docs.len(), 2);
        assert!(c2.strays.is_empty(), "{:?}", c2.strays);
        assert!(!r.join("docs/adr/0007-use-postgres.md").exists());
    }
}
