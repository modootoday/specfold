//! The graph derived from documents. It is computed on every run and never stored.

mod build;
pub mod query;
mod types;

pub use types::{DOC_RELATIONS, Edge, Graph, MENTIONS, Resolution};

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
    fn resolves_and_queries() {
        let tmp = tempfile::tempdir().unwrap();
        let r = tmp.path();
        std::fs::create_dir_all(r.join("apps/shop/src/cart")).unwrap();
        write(
            r,
            "apps/shop/.spec/sot/cart.sot.md",
            "---\nstatus: active\nrel:\n  governs: [\"apps/shop/src/cart/**\"]\n---\n# Cart\n## Totals {#CO-1}\n",
        );
        write(
            r,
            "apps/shop/.spec/adr/20260101000000-old.adr.md",
            "---\nstatus: superseded\naliases: [ADR-7]\n---\n# Old\n",
        );
        write(
            r,
            "apps/shop/.spec/adr/20260201000000-new.adr.md",
            "---\nstatus: active\nrel:\n  supersedes: [ADR-7]\n  implements: [\"sot:shop/cart#CO-1\"]\n---\n# New\n",
        );
        write(
            r,
            "apps/shop/.spec/plan/20260301000000-ship.plan.md",
            "---\nstatus: active\n---\n# Ship\n[[adr:shop/20260201000000]] [[sot:shop/cart#NOPE]]\n",
        );
        let mut raw = RawConfig::default();
        raw.owners.kinds = vec![OwnerKind { pattern: "apps/*".into(), id: "{1}".into(), manifest: false }];
        let c = Corpus::load(Config::from_raw(r.to_path_buf(), raw, false), &LoadOptions::default());
        let g = Graph::build(&c);
        let idx = |id: &str| g.by_id[id];
        assert!(matches!(g.resolve("ADR-7"), Resolution::Doc { via_alias: true, .. }));
        assert!(matches!(g.resolve("sot:shop/cart#NOPE"), Resolution::MissingAnchor { .. }));
        assert_eq!(
            query::chain(&g, idx("adr:shop/20260101000000")),
            [idx("adr:shop/20260101000000"), idx("adr:shop/20260201000000")]
        );
        let imp: Vec<usize> = query::impact(&g, idx("sot:shop/cart")).into_iter().map(|x| x.0).collect();
        assert_eq!(imp, [idx("adr:shop/20260201000000"), idx("plan:shop/20260301000000")]);
        let gov = query::governing(&c, "apps/shop/src/cart/total.rs");
        assert_eq!(gov[0].idx, idx("sot:shop/cart"));
        assert_eq!(gov.len(), 4);
    }
}
