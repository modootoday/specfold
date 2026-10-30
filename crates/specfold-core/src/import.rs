//! Importing documents from another layout through a project's import map.

pub mod body_meta;
pub mod convert;
pub mod key;
pub mod map;
pub mod owner;
pub mod plan;
pub mod presets;
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
        write(r, "old/notes.md", "# Notes\nA real document next to the stub.\n");
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
[[sources]]
glob = "old/*.md"
type = "guide"
"#,
        )
        .unwrap();
        let mut raw = RawConfig::default();
        raw.owners.kinds = vec![OwnerKind { pattern: "services/*".into(), id: "{1}".into(), manifest: false }];
        let c = Corpus::load(Config::from_raw(r.to_path_buf(), raw, false), &LoadOptions::default());
        let p = build(&c, &map).unwrap();
        let ids: Vec<&str> = p.rows.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "adr:shop/20260105000000",
                "research:root/20260201000000",
                "guide:root/notes",
                "research:root/20260201000000"
            ]
        );
        assert_eq!(p.rows[0].status_to, "active");
        assert_eq!(p.rows[0].aliases, ["0007"]);
        assert_eq!(p.rows[1].path, ".spec/research/20260201000000-launch/index.research.md");
        assert!(p.alias_rows.contains(&("old/launch.md".to_string(), "research:root/20260201000000".to_string())));
        let chapter =
            ("docs/plans/2026-02-01-launch/10-steps.md".to_string(), "research:root/20260201000000".to_string());
        assert!(p.alias_rows.contains(&chapter), "a chapter's old path names its document");
        p.plan.apply(r).unwrap();
        let c2 = Corpus::load(Config::from_raw(r.to_path_buf(), c.cfg.raw, false), &LoadOptions::default());
        assert_eq!(c2.docs.len(), 3);
        assert!(c2.strays.is_empty(), "{:?}", c2.strays);
        assert!(!r.join("docs/adr/0007-use-postgres.md").exists());
    }

    #[test]
    fn a_nested_bundle_belongs_to_the_first_rule_and_names_resolve_by_stem() {
        let tmp = tempfile::tempdir().unwrap();
        let r = tmp.path();
        write(r, "notes/catalog/2026-03-01-round/README.md", "# Round\n");
        write(r, "notes/catalog/2026-03-01-round/data.json", "{}");
        write(r, "notes/catalog/overview.md", "# Catalog\n");
        let map: ImportMap = toml::from_str(
            "stem_aliases = true\n[[sources]]\nglob = \"notes/*/*/\"\ntype = \"research\"\ndefault_status = \"active\"\n[[sources]]\nglob = \"notes/*/\"\ntype = \"research\"\ndefault_status = \"active\"\n",
        )
        .unwrap();
        let c = Corpus::load(Config::from_raw(r.to_path_buf(), RawConfig::default(), false), &LoadOptions::default());
        let p = build(&c, &map).unwrap();
        assert_eq!(p.rows.len(), 2);
        let outer =
            p.alias_rows.iter().filter(|(_, id)| id == &p.rows[1].id).map(|(old, _)| old.as_str()).collect::<Vec<_>>();
        assert!(
            !outer.iter().any(|o| o.contains("2026-03-01-round/")),
            "the outer bundle took nested files: {outer:?}"
        );
        assert!(p.alias_rows.contains(&("2026-03-01-round".to_string(), p.rows[0].id.clone())));
    }

    #[test]
    fn raw_files_are_wrapped_and_old_indexes_dropped() {
        let tmp = tempfile::tempdir().unwrap();
        let r = tmp.path();
        write(r, "raw/2026-01-02-run.json", "{}");
        write(r, "raw/2026-01-03-batch/a.csv", "x");
        write(r, "raw/2026-01-03-batch/b.csv", "y");
        write(r, "old/INDEX.md", "| generated |\n");
        let map: ImportMap = toml::from_str(
            "[[sources]]\nglob = \"old/INDEX.md\"\ndrop = true\n[[sources]]\nglob = \"raw/*/\"\ntype = \"evidence\"\nwrap = true\nkey = \"name\"\nname = '^(?P<key>\\d{4}-\\d{2}-\\d{2})-(?P<slug>.+)$'\n[[sources]]\nglob = \"raw/*\"\ntype = \"evidence\"\nwrap = true\nkey = \"name\"\nname = '^(?P<key>\\d{4}-\\d{2}-\\d{2})-(?P<slug>.+)\\.json$'\n",
        )
        .unwrap();
        let c = Corpus::load(Config::from_raw(r.to_path_buf(), RawConfig::default(), false), &LoadOptions::default());
        let p = build(&c, &map).unwrap();
        let kinds: Vec<&str> = p.rows.iter().map(|r| r.kind.as_str()).collect();
        assert_eq!(kinds, ["drop", "doc", "doc"]);
        p.plan.apply(r).unwrap();
        let c2 = Corpus::load(Config::from_raw(r.to_path_buf(), RawConfig::default(), false), &LoadOptions::default());
        assert_eq!(c2.docs.len(), 2);
        let batch = c2.docs.iter().find(|d| d.id.key == "20260103000000").unwrap();
        assert_eq!(batch.files().len(), 2);
        assert_eq!(batch.title.as_deref(), Some("batch"));
        assert!(!r.join("old/INDEX.md").exists() && !r.join("raw/2026-01-02-run.json").exists());
    }
}
