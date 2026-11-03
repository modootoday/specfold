//! The spec directories: one per owner that has one, plus the local directory; and which of
//! them holds a document.

use super::types::{Corpus, SpecRoot, Stray};
use crate::config::Config;
use crate::model::Tier;
use crate::workspace::Owners;

pub fn committed(cfg: &Config, owners: &Owners) -> Vec<SpecRoot> {
    let type_dirs = cfg.type_dirs();
    let former = crate::config::former_type_dirs::former(cfg);
    let mut out = vec![];
    for o in &owners.list {
        let spec = (owners.spec_dir(o, cfg.spec_dir()), None, false);
        let own = type_dirs.iter().map(|(d, t)| (owners.within(o, d), Some(t.clone()), false));
        let gone = former.iter().map(|(d, t)| (owners.within(o, d), Some(t.clone()), true));
        for (dir, ty, former) in std::iter::once(spec).chain(own).chain(gone) {
            let root = SpecRoot { owner: o.id.clone(), dir, tier: Tier::Committed, pending: o.pending, ty, former };
            if cfg.spec_home.join(&root.dir).is_dir() {
                out.push(root);
            }
        }
    }
    out
}

/// `.spec.local/<owner-id>/` directories. Unknown owner ids are reported as strays.
pub fn local(cfg: &Config, owners: &Owners, strays: &mut Vec<Stray>) -> Vec<SpecRoot> {
    let base = cfg.local_dir();
    let Ok(rd) = std::fs::read_dir(cfg.spec_home.join(base)) else { return vec![] };
    let mut out = vec![];
    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let rel = format!("{base}/{name}");
        if !entry.path().is_dir() {
            strays.push(Stray { path: rel, reason: "local files go in a directory named after their owner (.spec.local/<owner>/<type>/); create local documents with spec new <type> --local --write, and keep plain notes outside .spec.local".into() });
            continue;
        }
        if owners.by_id(&name).is_none() {
            strays.push(Stray { path: rel, reason: format!("'{name}' is not an owner id") });
            continue;
        }
        out.push(SpecRoot { owner: name, dir: rel, tier: Tier::Local, pending: false, ty: None, former: false });
    }
    out.sort_by(|a, b| a.dir.cmp(&b.dir));
    out
}

impl Corpus {
    /// The spec directory holding document `idx`: the deepest root its path lies under.
    pub fn spec_root_of(&self, idx: usize) -> Option<&SpecRoot> {
        let path = &self.docs[idx].path;
        self.roots.iter().filter(|r| crate::paths::is_below(path, &r.dir)).max_by_key(|r| r.dir.len())
    }
}
