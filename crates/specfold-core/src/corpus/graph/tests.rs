//! Unit tests for the graph: resolution and queries.

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
    std::fs::create_dir_all(r.join("services/shop/src/cart")).unwrap();
    write(
        r,
        "services/shop/.spec/sot/cart.sot.md",
        "---\nstatus: active\nrel:\n  governs: [\"services/shop/src/cart/**\"]\n---\n# Cart\n## Totals {#CO-1}\n",
    );
    write(
        r,
        "services/shop/.spec/adr/20260101000000-old.adr.md",
        "---\nstatus: superseded\naliases: [ADR-7]\n---\n# Old\n",
    );
    write(
        r,
        "services/shop/.spec/adr/20260201000000-new.adr.md",
        "---\nstatus: active\nrel:\n  supersedes: [ADR-7]\n  implements: [\"sot:shop/cart#CO-1\"]\n---\n# New\n",
    );
    write(
        r,
        "services/shop/.spec/research/20260301000000-ship.research.md",
        "---\nstatus: active\n---\n# Ship\n[[adr:shop/20260201000000]] [[sot:shop/cart#NOPE]]\n",
    );
    let mut raw = RawConfig::default();
    raw.owners.kinds = vec![OwnerKind { pattern: "services/*".into(), id: "{1}".into(), manifest: false }];
    let c = Corpus::load(Config::from_raw(r.to_path_buf(), raw, false), &LoadOptions::default());
    let g = Graph::build(&c);
    let idx = |id: &str| g.by_id[id];
    assert!(matches!(g.resolve("ADR-7"), Resolution::Doc { via_alias: true, .. }));
    let relative = g.resolve("../../services/shop/.spec/sot/cart.sot.md");
    assert!(matches!(relative, Resolution::Doc { idx, .. } if idx == g.by_id["sot:shop/cart"]));
    assert!(matches!(g.resolve("sot:shop/cart#NOPE"), Resolution::MissingAnchor { .. }));
    assert_eq!(
        query::chain(&g, idx("adr:shop/20260101000000")),
        [idx("adr:shop/20260101000000"), idx("adr:shop/20260201000000")]
    );
    let imp: Vec<usize> = query::impact(&g, idx("sot:shop/cart")).into_iter().map(|x| x.0).collect();
    assert_eq!(imp, [idx("adr:shop/20260201000000"), idx("research:shop/20260301000000")]);
    let gov = query::governing(&c, "services/shop/src/cart/total.rs");
    assert_eq!(gov[0].idx, idx("sot:shop/cart"));
    assert_eq!(gov.len(), 4);
}

#[test]
fn prefix_key_names_the_literal_folder_every_match_lies_in() {
    let cases: &[(&str, Option<&str>)] = &[
        ("services/shop", Some("services/shop")),
        ("services/shop/", Some("services/shop")),
        ("Cargo.toml", Some("Cargo.toml")),
        ("services/*/src/**", Some("services")),
        ("services/shop/src/**/*.rs", Some("services/shop/src")),
        ("services/sh*/src", Some("services")),
        ("*.toml", None),
        ("**/*.rs", None),
        ("serv*/src", None),
        ("{a,b}/src/**", None),
        ("a[[]b/src/*", None),
        ("a\\*b/src/*", None),
        ("services/[", None),
        ("", None),
        ("/", None),
        ("/*.rs", None),
    ];
    for (pattern, want) in cases {
        assert_eq!(super::governs::prefix_key(pattern).as_deref(), *want, "{pattern}");
    }
}

#[test]
fn governing_index_matches_every_compiled_claim_across_patterns() {
    let tmp = tempfile::tempdir().unwrap();
    let r = tmp.path();
    let patterns = [
        "services/shop",
        "services/shop/src/**/*.rs",
        "services/*/src/**",
        "*.toml",
        "**/README.md",
        "libs",
        "services/blog/",
        "{services,libs}/**",
        "services/[",
    ];
    for (n, g) in patterns.iter().enumerate() {
        write(
            r,
            &format!("services/shop/.spec/sot/g{n}.sot.md"),
            &format!("---\nrel:\n  governs: ['{g}']\n---\n# G\n"),
        );
    }
    write(r, "services/blog/.spec/sot/plain.sot.md", "---\nstatus: active\n---\n# Owner only\n");
    let files = ["libs/auth/README.md", "services/shop/src/cart/total.rs"];
    for f in files {
        write(r, f, "x\n");
    }
    let mut raw = RawConfig::default();
    raw.owners.kinds = vec![OwnerKind { pattern: "services/*".into(), id: "{1}".into(), manifest: false }];
    let c = Corpus::load(Config::from_raw(r.to_path_buf(), raw, false), &LoadOptions::default());
    let index = query::GoverningIndex::new(&c);
    let paths = [
        "services/shop/src/cart/total.rs",
        "services/shop/Cargo.toml",
        "services/blog/src/main.rs",
        "libs/auth/README.md",
        "libs/auth",
        "libs",
        "services",
        "services/shop/src",
        "README.md",
        "Cargo.toml",
        "other/x.rs",
        "./services/blog/",
    ];
    for path in paths {
        let p = path.trim_start_matches("./").trim_end_matches('/');
        let mut want: Vec<usize> = vec![];
        for (i, d) in c.docs.iter().enumerate() {
            let globs = d.rel().remove("governs").unwrap_or_default();
            let folder = r.join(p).is_dir();
            let hit = if globs.is_empty() {
                crate::paths::is_under(p, "services/blog") || (folder && crate::paths::is_below("services/blog", p))
            } else {
                globs.iter().any(|g| {
                    let lit = g.split(['*', '?', '[', '{']).next().unwrap_or("");
                    let m = crate::workspace::owners::matcher(g);
                    let inside =
                        files.iter().any(|f| crate::paths::is_below(f, p) && m.as_ref().is_some_and(|m| m.is_match(f)));
                    m.as_ref().is_some_and(|m| m.is_match(p)) || (folder && (crate::paths::is_below(lit, p) || inside))
                })
            };
            if hit {
                want.push(i);
            }
        }
        let mut got: Vec<usize> = index.governing(&c, path).into_iter().map(|g| g.idx).collect();
        got.sort_unstable();
        assert_eq!(got, want, "{path}");
    }
}
