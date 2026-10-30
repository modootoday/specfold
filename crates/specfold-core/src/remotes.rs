//! Documents in other repositories. `[remotes.<name>] path = "../shared"` lets a reference or a
//! relation name a document there as `<name>:<id>` (`shared:adr:org/20261030000000`). The other
//! repository is read from a directory already on disk and never written. A remote whose
//! directory is absent (a CI job without that checkout) is skipped, not reported; a remote that
//! is present but lacks the id is an error like any missing document.

use crate::config::Config;
use crate::corpus::{Corpus, LoadOptions};
use crate::graph::{Graph, Resolution};
use crate::model::RefText;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

/// What `<name>:<id>` names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Remote {
    /// Not a remote reference: no configured remote of that name.
    Local,
    /// The remote is configured but its directory is not on disk.
    Absent {
        name: String,
        path: PathBuf,
    },
    /// One document: its id, type, title and absolute file path.
    Found {
        name: String,
        id: String,
        ty: String,
        title: String,
        file: PathBuf,
    },
    Missing {
        name: String,
        id: String,
    },
    MissingAnchor {
        name: String,
        id: String,
        anchor: String,
    },
    Ambiguous {
        name: String,
        ids: Vec<String>,
    },
}

pub struct Loaded {
    pub corpus: Corpus,
    pub graph: Graph,
}

type Cache = Mutex<HashMap<PathBuf, Option<Arc<Loaded>>>>;

fn cache() -> &'static Cache {
    static C: OnceLock<Cache> = OnceLock::new();
    C.get_or_init(Default::default)
}

/// The remote a reference names, and the rest of the reference. A configured type name is never a
/// remote, so `adr:x/y` stays a local id even with a remote called `adr`.
pub fn split<'a>(cfg: &Config, text: &'a str) -> Option<(&'a str, &'a str)> {
    let (name, rest) = text.trim().split_once(':')?;
    (cfg.raw.remotes.contains_key(name) && cfg.registry.get(name).is_none()).then_some((name, rest))
}

/// The remote's directory.
pub fn dir(cfg: &Config, name: &str) -> Option<PathBuf> {
    cfg.raw.remotes.get(name).map(|r| cfg.expand_path(&r.path))
}

/// Loads the remote corpus once per process; `None` when the directory is absent.
pub fn load(cfg: &Config, name: &str) -> Option<Arc<Loaded>> {
    let dir = dir(cfg, name)?;
    let mut c = cache().lock().unwrap_or_else(|e| e.into_inner());
    c.entry(dir.clone())
        .or_insert_with(|| {
            if !dir.is_dir() {
                return None;
            }
            let rc = Config::discover(&dir).ok()?;
            let corpus = Corpus::load(rc, &LoadOptions { include_local: false });
            let graph = Graph::build(&corpus);
            Some(Arc::new(Loaded { corpus, graph }))
        })
        .clone()
}

/// Resolves `<name>:<id>[#anchor]` against the remote repository.
pub fn resolve(cfg: &Config, text: &str) -> Remote {
    let Some((name, rest)) = split(cfg, text) else { return Remote::Local };
    let name = name.to_string();
    let Some(l) = load(cfg, &name) else {
        return Remote::Absent { path: dir(cfg, &name).unwrap_or_default(), name };
    };
    let id = RefText::parse(rest).target;
    match l.graph.resolve(rest) {
        Resolution::Doc { idx, .. } => {
            let d = &l.corpus.docs[idx];
            Remote::Found {
                name,
                id: d.id.to_string(),
                ty: d.id.ty.clone(),
                title: d.title.clone().unwrap_or_default(),
                file: l.corpus.cfg.root.join(&d.path),
            }
        }
        Resolution::MissingAnchor { anchor, .. } => Remote::MissingAnchor { name, id, anchor },
        Resolution::Ambiguous(list) => {
            Remote::Ambiguous { name, ids: list.iter().map(|&i| l.corpus.docs[i].id.to_string()).collect() }
        }
        Resolution::Missing => Remote::Missing { name, id },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::RawConfig;

    fn write(p: &std::path::Path, s: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, s).unwrap();
    }

    #[test]
    fn resolves_ids_in_a_sibling_repository() {
        let tmp = tempfile::tempdir().unwrap();
        let shared = tmp.path().join("shared");
        write(&shared.join("spec.toml"), "[spec]\nschema = 1\n");
        write(
            &shared.join(".spec/guide/setup.guide.md"),
            "---\nid: guide:root/setup\ntitle: Setup\nstatus: active\n---\n\n# Setup\n\n## Install {#install}\n",
        );
        let mut raw = RawConfig::default();
        raw.remotes.insert("org".into(), crate::config::types::RemoteRoot { path: shared.to_string_lossy().into() });
        raw.remotes.insert("gone".into(), crate::config::types::RemoteRoot { path: "/nonexistent/x".into() });
        let cfg = Config::from_raw(tmp.path().join("app"), raw, false);
        assert_eq!(resolve(&cfg, "guide:x/y"), Remote::Local);
        match resolve(&cfg, "org:guide:root/setup#install") {
            Remote::Found { id, ty, .. } => assert_eq!((id.as_str(), ty.as_str()), ("guide:root/setup", "guide")),
            other => panic!("{other:?}"),
        }
        assert!(matches!(resolve(&cfg, "org:guide:root/nope"), Remote::Missing { .. }));
        assert!(matches!(resolve(&cfg, "org:guide:root/setup#zzz"), Remote::MissingAnchor { .. }));
        assert!(matches!(resolve(&cfg, "gone:guide:a/b"), Remote::Absent { .. }));
    }
}
