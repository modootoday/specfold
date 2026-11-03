//! Questions about remote references: which remote a reference names, what it resolves to, its
//! source revision and status, and who names a document from elsewhere.

use super::load::{dir, load};
use super::types::{Consumer, Loaded, Remote};
use crate::config::Config;
use crate::corpus::Corpus;
use crate::graph::Resolution;
use crate::model::RefText;
use std::sync::Arc;

/// The remote a reference names, and the rest of the reference. A configured type name is never a
/// remote, so `adr:x/y` stays a local id even with a remote called `adr`.
pub fn split<'a>(cfg: &Config, text: &'a str) -> Option<(&'a str, &'a str)> {
    let (name, rest) = text.trim().split_once(':')?;
    (cfg.raw.remotes.contains_key(name) && cfg.registry.get(name).is_none()).then_some((name, rest))
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
                file: l.corpus.cfg.spec_home.join(&d.path),
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
        let external = |g: &str| crate::workspace::projects::is_external(&cfg, g);
        assert!(external("org:src/lib.rs") && !external("src/lib.rs"));
    }
}
