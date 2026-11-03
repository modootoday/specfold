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

/// The remote's directory: the first of `path` and `paths` on disk, else `path`.
pub fn dir(cfg: &Config, name: &str) -> Option<PathBuf> {
    let r = cfg.raw.remotes.get(name)?;
    let first = cfg.expand_path(&r.path);
    Some(r.paths.iter().map(|p| cfg.expand_path(p)).find(|p| p.is_dir()).filter(|_| !first.is_dir()).unwrap_or(first))
}

/// A governed path that belongs to another project: written `<remote>:<path>`, or inside a
/// remote checked out within this repository, or inside a submodule no owner kind claims. P1 and
/// L5 leave it out; D1 still matches it.
pub fn is_external(cfg: &Config, governs: &str) -> bool {
    if split(cfg, governs).is_some() {
        return true;
    }
    let g = governs.trim_start_matches("./").trim_end_matches('/');
    let under = |p: &str| {
        let p = p.trim_start_matches("./").trim_end_matches('/');
        !p.is_empty() && crate::paths::is_under(g, p)
    };
    inner_projects(cfg).iter().any(|p| under(p))
}

/// A1: the auto-registered submodule remote whose directory holds `rel`, and the path inside it.
pub fn submodule_at(cfg: &Config, rel: &str) -> Option<(String, String)> {
    cfg.raw.remotes.iter().filter(|(_, r)| r.auto).find_map(|(name, r)| {
        let p = r.path.trim_end_matches('/');
        let inner = if rel == p { "" } else { rel.strip_prefix(p)?.strip_prefix('/')? };
        Some((name.clone(), inner.to_string()))
    })
}

/// Whether the submodule at `path` is a project of its own rather than code of an owner here. A
/// submodule an owner kind claims is that owner's code, with or without its own spec.toml (a
/// product split into its own repository); any other submodule is vendored or another project.
fn own_project(owners: &crate::workspace::Owners, path: &str) -> bool {
    owners.by_path(path).is_none()
}

/// Projects inside this repository: submodules of their own and remotes checked out here, once
/// per root.
fn inner_projects(cfg: &Config) -> Vec<String> {
    static C: OnceLock<Mutex<HashMap<PathBuf, Vec<String>>>> = OnceLock::new();
    let mut c = C.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    c.entry(cfg.root.clone())
        .or_insert_with(|| {
            let owners = crate::workspace::Owners::discover(cfg);
            let mut v: Vec<String> =
                crate::workspace::vcs::gitlinks(&cfg.root).into_iter().filter(|p| own_project(&owners, p)).collect();
            v.extend(cfg.raw.owners.nested_dirs().map(String::from));
            // Submodules registered as remotes by `submodule_remotes` were judged above.
            for r in cfg.raw.remotes.values().filter(|r| !r.auto) {
                for p in std::iter::once(&r.path).chain(&r.paths) {
                    if !p.starts_with("..") && !p.starts_with('/') && !p.starts_with('~') {
                        v.push(p.clone());
                    }
                }
            }
            v
        })
        .clone()
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
            let start = std::time::Instant::now();
            let rc = Config::discover(&dir).ok()?;
            let corpus = Corpus::load(rc, &LoadOptions { include_local: false });
            let graph = Graph::build(&corpus);
            if crate::corpus::trace::enabled() {
                let ms = start.elapsed().as_secs_f64() * 1000.0;
                eprintln!("specfold:   remote {name:<12} {ms:>7.1} ms, {} documents", corpus.docs.len());
            }
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

/// The id and source revision (see `translation::rev_of`) of the remote document `<name>:<id>`,
/// or `None` when it is not a remote reference that resolves.
pub fn rev(cfg: &Config, text: &str) -> Option<(String, String)> {
    let (name, rest) = split(cfg, text)?;
    let l = load(cfg, name)?;
    match l.graph.resolve(rest) {
        Resolution::Doc { idx, .. } => {
            Some((format!("{name}:{}", l.corpus.docs[idx].id), crate::translation::rev_of(&l.corpus, idx)?))
        }
        _ => None,
    }
}
/// F2: the status of the remote document `<name>:<id>` and, when another document there
/// supersedes it, that document's id (`<name>:<id>`).
pub fn status(cfg: &Config, text: &str) -> Option<(String, Option<String>)> {
    let (name, rest) = split(cfg, text)?;
    let l = load(cfg, name)?;
    let Resolution::Doc { idx, .. } = l.graph.resolve(rest) else { return None };
    let me = l.corpus.docs[idx].id.to_string();
    let by = l.corpus.docs.iter().find(|d| d.rel().get("supersedes").is_some_and(|v| v.contains(&me)));
    Some((l.corpus.docs[idx].status().unwrap_or_default(), by.map(|d| format!("{name}:{}", d.id))))
}

/// A document in this repository or a remote that names another repository's document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Consumer {
    /// `.` for this repository, else the remote's name.
    pub repo: String,
    pub id: String,
    pub path: String,
    /// `rel.<kind>` or `pins`.
    pub via: String,
}

/// The documents, here and in every remote on disk, whose relations or pins name the document at
/// `target` (an absolute file path) through one of their own remotes. Only frontmatter is read;
/// a remote's own remotes are resolved from directories already on disk, never fetched.
pub fn consumers(cfg: &Config, here: &Corpus, target: &std::path::Path) -> Vec<Consumer> {
    let canon = |p: &std::path::Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let target = canon(target);
    let mut repos: Vec<(String, Arc<Loaded>)> = vec![];
    for name in cfg.raw.remotes.keys() {
        if let Some(l) = load(cfg, name) {
            repos.push((name.clone(), l));
        }
    }
    let mut out = vec![];
    let mut scan = |repo: &str, c: &Corpus| {
        for d in &c.docs {
            let mut refs: Vec<(String, String)> =
                d.rel().into_iter().flat_map(|(k, vs)| vs.into_iter().map(move |v| (format!("rel.{k}"), v))).collect();
            if let Some(serde_json::Value::Object(m)) = d.fm().and_then(|m| m.get("pins")) {
                refs.extend(m.keys().map(|k| ("pins".to_string(), k.clone())));
            }
            for (via, v) in refs {
                if let Remote::Found { file, .. } = resolve(&c.cfg, &v)
                    && canon(&file) == target
                {
                    let u = Consumer { repo: repo.to_string(), id: d.id.to_string(), path: d.path.clone(), via };
                    if !out.contains(&u) {
                        out.push(u);
                    }
                }
            }
        }
    };
    scan(".", here);
    for (name, l) in &repos {
        scan(name, &l.corpus);
    }
    out
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
        raw.remotes.insert(
            "org".into(),
            crate::config::types::RemoteRoot { path: shared.to_string_lossy().into(), paths: vec![], auto: false },
        );
        raw.remotes.insert(
            "gone".into(),
            crate::config::types::RemoteRoot {
                path: "/nonexistent/x".into(),
                paths: vec![shared.to_string_lossy().into()],
                auto: false,
            },
        );
        let cfg = Config::from_raw(tmp.path().join("app"), raw, false);
        assert_eq!(resolve(&cfg, "guide:x/y"), Remote::Local);
        match resolve(&cfg, "org:guide:root/setup#install") {
            Remote::Found { id, ty, .. } => assert_eq!((id.as_str(), ty.as_str()), ("guide:root/setup", "guide")),
            other => panic!("{other:?}"),
        }
        assert!(matches!(resolve(&cfg, "org:guide:root/nope"), Remote::Missing { .. }));
        assert!(matches!(resolve(&cfg, "org:guide:root/setup#zzz"), Remote::MissingAnchor { .. }));
        assert!(matches!(resolve(&cfg, "gone:guide:root/setup"), Remote::Found { .. }), "a later candidate path");
        assert!(is_external(&cfg, "org:src/lib.rs") && !is_external(&cfg, "src/lib.rs"));
    }
}
