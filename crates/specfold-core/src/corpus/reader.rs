//! Loads the corpus: owners, spec roots, documents, strays and aliases.

use super::cache::ScanCache;
use super::types::{Corpus, LoadOptions, SpecRoot};
use super::{aliases, assemble, roots, walk};
use crate::config::Config;
use crate::markdown::body;
use crate::trace;
use crate::workspace::Owners;

impl Corpus {
    pub fn load(cfg: Config, opts: &LoadOptions) -> Self {
        let t = std::time::Instant::now();
        let owners = Owners::discover(&cfg);
        let t = trace::phase("owners", t);
        let mut strays = vec![];
        let mut all_roots: Vec<SpecRoot> = roots::committed(&cfg, &owners);
        if opts.include_local {
            all_roots.extend(roots::local(&cfg, &owners, &mut strays));
        }
        let mut pending = vec![];
        for root in &all_roots {
            let files = walk::files(&cfg.root, &root.dir, root.tier);
            pending.extend(assemble::group(&cfg, root, &files, &mut strays));
        }
        let t = trace::phase("walk", t);
        let to_scan: Vec<String> =
            pending.iter().flat_map(|p| std::iter::once(p.main.clone()).chain(p.chapters.iter().cloned())).collect();
        let mut cache = ScanCache::open(&cfg);
        let t = trace::phase("cache-open", t);
        let mut scans = cache.scan_all(&cfg.root, &to_scan, &body::path_cite_re(cfg.spec_dir()));
        let t = trace::phase("scan", t);
        cache.save();
        let t = trace::phase("cache-save", t);
        let mut docs: Vec<_> = pending.into_iter().filter_map(|p| assemble::build(p, &mut scans)).collect();
        docs.sort_by(|a, b| a.path.cmp(&b.path));
        strays.sort_by(|a, b| a.path.cmp(&b.path));
        let aliases = aliases::load(&cfg);
        trace::phase("assemble", t);
        Corpus { cfg, owners, roots: all_roots, docs, strays, aliases }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{OwnerKind, RawConfig};

    pub fn write(root: &std::path::Path, rel: &str, text: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    #[test]
    fn loads_owners_docs_and_strays() {
        let tmp = tempfile::tempdir().unwrap();
        let r = tmp.path();
        write(
            r,
            ".spec/adr/20260101000000-pick-db.adr.md",
            "---\nid: adr:ws/20260101000000\nstatus: active\n---\n# Pick the database\nSee [[sot:shop/cart]].\n",
        );
        write(r, "apps/shop/.spec/sot/cart.sot.md", "---\nstatus: active\n---\n# Cart\n## Totals {#CO-1}\n");
        write(
            r,
            "apps/shop/.spec/design/20260102000000-cart/index.design.md",
            "---\nstatus: draft\n---\n# Cart boards\n",
        );
        write(r, "apps/shop/.spec/design/20260102000000-cart/assets/a.webp", "x");
        write(r, "apps/shop/.spec/plans/old.md", "legacy");
        write(r, ".spec.local/shop/plan/20260103000000-draft.plan.md", "# Draft\n");
        let mut raw = RawConfig::default();
        raw.owners.root = "ws".into();
        raw.owners.kinds = vec![OwnerKind { pattern: "apps/*".into(), id: "{1}".into(), manifest: false }];
        let cfg = Config::from_raw(r.to_path_buf(), raw, false);
        let c = Corpus::load(cfg, &LoadOptions { include_local: true });
        let ids: Vec<String> = c.docs.iter().map(|d| d.id.to_string()).collect();
        assert_eq!(
            ids,
            ["plan:shop/20260103000000", "adr:ws/20260101000000", "design:shop/20260102000000", "sot:shop/cart"]
        );
        assert_eq!(c.strays.len(), 1);
        assert_eq!(c.strays[0].path, "apps/shop/.spec/plans/old.md");
        let design = c.docs.iter().find(|d| d.id.ty == "design").unwrap();
        assert_eq!(design.bundle_files, ["apps/shop/.spec/design/20260102000000-cart/assets/a.webp"]);
        let adr = c.docs.iter().find(|d| d.id.ty == "adr").unwrap();
        assert_eq!(adr.links[0].item, "sot:shop/cart");
        assert_eq!(adr.title.as_deref(), Some("Pick the database"));
    }
}
