//! Read-only questions about owners: which owner contains a path, where an owner's documents
//! are stored.

use super::types::{Owner, Owners};

impl Owners {
    pub fn root(&self) -> &Owner {
        &self.list[0]
    }

    pub fn by_id(&self, id: &str) -> Option<&Owner> {
        self.list.iter().find(|o| o.id == id)
    }

    pub fn by_path(&self, path: &str) -> Option<&Owner> {
        let p = path.trim_matches('/');
        self.list.iter().find(|o| o.path == p)
    }

    /// The narrowest owner whose directory contains `rel` (a repository-relative path).
    pub fn containing(&self, rel: &str) -> &Owner {
        let rel = rel.trim_start_matches("./");
        self.list
            .iter()
            .filter(|o| o.path.is_empty() || rel == o.path || rel.starts_with(&format!("{}/", o.path)))
            .max_by_key(|o| o.path.len())
            .unwrap_or(self.root())
    }

    pub fn is_public_path(&self, rel: &str) -> bool {
        self.public.iter().any(|m| m.is_match(rel))
    }

    /// Where documents of `owner` are stored: the owner itself, or its private companion.
    pub fn storage_owner<'a>(&'a self, owner: &'a Owner) -> &'a Owner {
        if !owner.public {
            return owner;
        }
        owner.companion.as_deref().and_then(|c| self.by_path(c)).unwrap_or(self.root())
    }

    /// The spec directory of an owner, relative to the repository root.
    pub fn spec_dir(&self, owner: &Owner, dir: &str) -> String {
        if owner.path.is_empty() { dir.to_string() } else { format!("{}/{dir}", owner.path) }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{Config, OwnerKind, RawConfig};
    use crate::workspace::owners::Owners;

    #[test]
    fn discovers_and_contains() {
        let tmp = tempfile::tempdir().unwrap();
        let r = tmp.path();
        for d in ["apps/shop/src", "libs/auth", "libs/notes", "libs/open"] {
            std::fs::create_dir_all(r.join(d)).unwrap();
        }
        std::fs::write(r.join("libs/auth/Cargo.toml"), "").unwrap();
        std::fs::write(r.join("libs/open/go.mod"), "").unwrap();
        let mut raw = RawConfig::default();
        raw.owners.root = "ws".into();
        raw.owners.kinds = vec![
            OwnerKind { pattern: "apps/*".into(), id: "{1}".into(), manifest: false },
            OwnerKind { pattern: "libs/*".into(), id: "lib.{1}".into(), manifest: true },
        ];
        raw.public.paths = vec!["libs/open".into()];
        raw.public.companion.insert("libs/open".into(), "apps/shop".into());
        let cfg = Config::from_raw(r.to_path_buf(), raw, false);
        let o = Owners::discover(&cfg);
        let ids: Vec<_> = o.list.iter().map(|x| x.id.as_str()).collect();
        assert_eq!(ids, ["ws", "shop", "lib.auth", "lib.open"]);
        assert_eq!(o.containing("apps/shop/src/main.rs").id, "shop");
        assert_eq!(o.containing("libs/notes/x").id, "ws");
        let open = o.by_id("lib.open").unwrap();
        assert!(open.public);
        assert_eq!(o.storage_owner(open).id, "shop");
    }
}
