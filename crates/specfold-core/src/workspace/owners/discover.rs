//! Builds the owner list from the configured kinds.

use super::patterns::{PathMatch, expand, has_manifest, matcher, render_id};
use super::types::{Owner, Owners};
use crate::config::Config;

impl Owners {
    pub fn discover(cfg: &Config) -> Self {
        let manifests = cfg.manifests();
        let public: Vec<PathMatch> = cfg.raw.public.paths.iter().filter_map(|p| matcher(p)).collect();
        let retired: Vec<PathMatch> = cfg.raw.owners.retired.iter().filter_map(|p| matcher(p)).collect();
        let mut list = vec![Owner {
            id: cfg.root_owner().to_string(),
            path: String::new(),
            kind: String::new(),
            public: false,
            companion: None,
            retired: false,
        }];
        for kind in &cfg.raw.owners.kinds {
            for (path, caps) in expand(&cfg.root, &kind.pattern) {
                if kind.manifest && !has_manifest(&cfg.root.join(&path), &manifests) {
                    continue;
                }
                if list.iter().any(|o| o.path == path) {
                    continue;
                }
                list.push(Owner {
                    id: render_id(&kind.id, &caps),
                    public: public.iter().any(|m| m.is_match(&path)),
                    companion: cfg.raw.public.companion.get(&path).cloned(),
                    retired: retired.iter().any(|m| m.is_match(&path)),
                    kind: kind.pattern.clone(),
                    path,
                });
            }
        }
        Owners { list, public }
    }
}
