//! Reading a remote repository from the directory it is checked out in, once per process.

use super::types::Loaded;
use crate::config::Config;
use crate::corpus::graph::Graph;
use crate::corpus::{Corpus, LoadOptions};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

type Cache = Mutex<HashMap<PathBuf, Option<Arc<Loaded>>>>;

fn cache() -> &'static Cache {
    static C: OnceLock<Cache> = OnceLock::new();
    C.get_or_init(Default::default)
}

/// The remote's directory: the first of `path` and `paths` on disk, else `path`.
pub fn dir(cfg: &Config, name: &str) -> Option<PathBuf> {
    let r = cfg.raw.remotes.get(name)?;
    let first = cfg.expand_path(&r.path);
    Some(r.paths.iter().map(|p| cfg.expand_path(p)).find(|p| p.is_dir()).filter(|_| !first.is_dir()).unwrap_or(first))
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
