//! Finds the spec directories: one per owner that has one, plus the local directory.

use super::types::{SpecRoot, Stray};
use crate::config::Config;
use crate::model::Tier;
use crate::workspace::Owners;

pub fn committed(cfg: &Config, owners: &Owners) -> Vec<SpecRoot> {
    owners
        .list
        .iter()
        .filter_map(|o| {
            let dir = owners.spec_dir(o, cfg.spec_dir());
            cfg.root.join(&dir).is_dir().then(|| SpecRoot { owner: o.id.clone(), dir, tier: Tier::Committed })
        })
        .collect()
}

/// `.spec.local/<owner-id>/` directories. Unknown owner ids are reported as strays.
pub fn local(cfg: &Config, owners: &Owners, strays: &mut Vec<Stray>) -> Vec<SpecRoot> {
    let base = cfg.local_dir();
    let Ok(rd) = std::fs::read_dir(cfg.root.join(base)) else { return vec![] };
    let mut out = vec![];
    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let rel = format!("{base}/{name}");
        if !entry.path().is_dir() {
            strays.push(Stray { path: rel, reason: "local files go in a directory named after their owner".into() });
            continue;
        }
        if owners.by_id(&name).is_none() {
            strays.push(Stray { path: rel, reason: format!("'{name}' is not an owner id") });
            continue;
        }
        out.push(SpecRoot { owner: name, dir: rel, tier: Tier::Local });
    }
    out.sort_by(|a, b| a.dir.cmp(&b.dir));
    out
}
