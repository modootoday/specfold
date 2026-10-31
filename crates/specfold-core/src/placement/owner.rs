//! Which owner a document belongs to, computed from the code it governs.

use crate::workspace::{Owner, Owners};

#[derive(Debug, Clone)]
pub struct Decision<'a> {
    pub owner: &'a Owner,
    /// Why this owner: shown to the user and to agents.
    pub reason: String,
}

/// The directory part of a path or glob before its first wildcard.
pub fn literal_base(glob: &str) -> String {
    let g = glob.trim_start_matches("./");
    // C185: `[[]a]` is the literal directory `[a]`, not a wildcard.
    let (head, wild) = crate::workspace::literal_prefix(g);
    if !wild {
        return head.trim_end_matches('/').to_string();
    }
    match head.rsplit_once('/') {
        Some((dir, _)) => dir.to_string(),
        None => String::new(),
    }
}

fn common_dir(a: &str, b: &str) -> String {
    let mut out = vec![];
    for (x, y) in a.split('/').zip(b.split('/')) {
        if x != y || x.is_empty() {
            break;
        }
        out.push(x);
    }
    out.join("/")
}

/// The narrowest owner containing every governed path, moved to its private companion when it is
/// public and to the root when its code is retired. Without `governs`, `current` stays.
pub fn decide<'a>(owners: &'a Owners, governs: &[String], current: &'a Owner) -> Decision<'a> {
    if governs.is_empty() {
        return Decision {
            owner: owners.storage_owner(current),
            reason: "no rel.governs; stays with its owner".into(),
        };
    }
    let mut owner_ids: Vec<&Owner> = governs.iter().map(|g| owners.containing(&literal_base(g))).collect();
    owner_ids.dedup_by(|a, b| a.id == b.id);
    let owner = if owner_ids.iter().all(|o| o.id == owner_ids[0].id) {
        owner_ids[0]
    } else {
        let common = owner_ids.iter().skip(1).fold(owner_ids[0].path.clone(), |acc, o| common_dir(&acc, &o.path));
        owners.containing(&common)
    };
    let mut reason = if owner_ids.len() > 1 {
        format!("governs code of {} owners; their common owner is {}", owner_ids.len(), owner.id)
    } else {
        format!("governs code inside {}", if owner.path.is_empty() { "the repository root" } else { &owner.path })
    };
    if owner.retired {
        return Decision { owner: owners.root(), reason: format!("{reason}, which is retired") };
    }
    let stored = owners.storage_owner(owner);
    if stored.id != owner.id {
        reason = format!("{reason}, which is public; documents go to {}", stored.id);
    }
    Decision { owner: stored, reason }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, OwnerKind, RawConfig};

    #[test]
    fn decides() {
        let tmp = tempfile::tempdir().unwrap();
        let r = tmp.path();
        for d in ["services/shop/src", "services/blog", "libs/open", "services/old"] {
            std::fs::create_dir_all(r.join(d)).unwrap();
        }
        let mut raw = RawConfig::default();
        raw.owners.kinds = vec![
            OwnerKind { pattern: "services/*".into(), id: "{1}".into(), manifest: false },
            OwnerKind { pattern: "libs/*".into(), id: "lib.{1}".into(), manifest: false },
        ];
        raw.owners.retired = vec!["services/old".into()];
        raw.public.paths = vec!["libs/open".into()];
        raw.public.companion.insert("libs/open".into(), "services/shop".into());
        let cfg = Config::from_raw(r.to_path_buf(), raw, false);
        let o = Owners::discover(&cfg);
        let root = o.root();
        let gov = |g: &[&str]| decide(&o, &g.iter().map(|s| s.to_string()).collect::<Vec<_>>(), root).owner.id.clone();
        assert_eq!(gov(&["services/shop/src/**"]), "shop");
        assert_eq!(gov(&["services/shop/src/**", "services/blog/**"]), "root");
        assert_eq!(gov(&["libs/open/src/**"]), "shop");
        assert_eq!(gov(&["services/old/**"]), "root");
        assert_eq!(literal_base("services/shop/src/*.rs"), "services/shop/src");
        assert_eq!(literal_base("services/shop/s*"), "services/shop");
    }
}
