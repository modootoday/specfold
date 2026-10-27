//! A census of the corpus: how much there is, where, and in what state. Used to measure a
//! migration before and after, and to confirm an old layout is gone.

use crate::corpus::Corpus;
use crate::graph::{Graph, Resolution};
use crate::workspace::owners::matcher;
use ignore::WalkBuilder;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Serialize)]
pub struct Census {
    pub docs: usize,
    pub by_root: BTreeMap<String, usize>,
    pub by_type: BTreeMap<String, usize>,
    pub by_status: BTreeMap<String, usize>,
    pub owners_with_docs: usize,
    pub strays: usize,
    pub links: usize,
    pub links_broken: usize,
    pub links_via_alias: usize,
    pub path_cites: usize,
    pub attachments: usize,
    pub attachment_bytes: u64,
    pub aliases: usize,
    /// Files matching `[forbid] paths`: zero once an old layout is fully retired.
    pub legacy_paths: usize,
}

pub fn take(c: &Corpus, g: &Graph) -> Census {
    let mut by_root = BTreeMap::new();
    let mut by_type = BTreeMap::new();
    let mut by_status = BTreeMap::new();
    let (mut links, mut broken, mut via_alias, mut cites, mut attachments, mut bytes) = (0, 0, 0, 0, 0, 0u64);
    for d in &c.docs {
        let root = c
            .roots
            .iter()
            .find(|r| d.path.starts_with(&format!("{}/", r.dir)))
            .map(|r| r.dir.clone())
            .unwrap_or_default();
        *by_root.entry(root).or_insert(0) += 1;
        *by_type.entry(d.id.ty.clone()).or_insert(0) += 1;
        *by_status.entry(d.status().unwrap_or_else(|| "(none)".into())).or_insert(0) += 1;
        for l in &d.links {
            links += 1;
            match g.resolve(&l.item) {
                Resolution::Doc { via_alias: true, .. } => via_alias += 1,
                Resolution::Doc { .. } => {}
                _ => broken += 1,
            }
        }
        cites += d.path_cites.len();
        attachments += d.bundle_files.len();
        bytes += d
            .bundle_files
            .iter()
            .filter_map(|f| std::fs::metadata(c.cfg.root.join(f)).ok())
            .map(|m| m.len())
            .sum::<u64>();
    }
    let forbid: Vec<_> = c.cfg.raw.forbid.paths.iter().filter_map(|p| matcher(p)).collect();
    let legacy_paths = if forbid.is_empty() {
        0
    } else {
        WalkBuilder::new(&c.cfg.root)
            .hidden(false)
            .require_git(false)
            .build()
            .flatten()
            .filter_map(|e| e.path().strip_prefix(&c.cfg.root).ok().map(|p| p.to_string_lossy().replace('\\', "/")))
            .filter(|p| !p.is_empty() && forbid.iter().any(|m| m.is_match(p)))
            .count()
    };
    Census {
        docs: c.docs.len(),
        owners_with_docs: by_root.len(),
        by_root,
        by_type,
        by_status,
        strays: c.strays.len(),
        links,
        links_broken: broken,
        links_via_alias: via_alias,
        path_cites: cites,
        attachments,
        attachment_bytes: bytes,
        aliases: g.by_alias.len(),
        legacy_paths,
    }
}
