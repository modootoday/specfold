//! Loads configuration, corpus and graph once per command, only as far as the command needs.

use crate::cli::args::global::Global;
use anyhow::{Result, bail};
use specfold_core::graph::Resolution;
use specfold_core::{Config, Corpus, Graph, LoadOptions};
use std::path::PathBuf;

pub struct Session {
    pub corpus: Corpus,
    pub graph: Graph,
}

/// The configuration, with a warning on standard error for `[types.<name>]` tables that look
/// like typos (doctor lists them as findings instead).
pub fn config(g: &Global) -> Result<Config> {
    let cfg = config_quiet(g)?;
    if !WARNED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        for w in specfold_core::model::registry::suspect_overrides(&cfg.raw.types) {
            eprintln!("warning: spec.toml: {w}");
        }
    }
    Ok(cfg)
}

/// Set once the config warnings were shown, or by a command that reports them itself.
static WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn hold_config_warnings() {
    WARNED.store(true, std::sync::atomic::Ordering::Relaxed);
}

pub fn config_quiet(g: &Global) -> Result<Config> {
    let dir = match &g.dir {
        Some(d) => d.clone(),
        None => std::env::current_dir()?,
    };
    Config::discover(&dir)
}

pub fn open(g: &Global) -> Result<Session> {
    let cfg = config(g)?;
    let corpus = Corpus::load(cfg, &LoadOptions { include_local: !g.no_local });
    let t = std::time::Instant::now();
    let graph = Graph::build(&corpus);
    specfold_core::trace::phase("graph", t);
    Ok(Session { corpus, graph })
}

/// A path given on the command line, made relative to the repository root.
pub fn repo_path(s: &Session, p: &str) -> String {
    let path = PathBuf::from(p);
    let abs = if path.is_absolute() { path } else { std::env::current_dir().map(|d| d.join(&path)).unwrap_or(path) };
    match abs.canonicalize().ok().and_then(|a| a.strip_prefix(&s.corpus.cfg.root).ok().map(|r| r.to_path_buf())) {
        Some(r) => r.to_string_lossy().replace('\\', "/"),
        None => p.trim_start_matches("./").trim_end_matches('/').to_string(),
    }
}

/// A word quoted for a shell when it needs to be.
pub fn shell_word(s: &str) -> String {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "-_./:#@".contains(c)) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

/// Resolves a reference to one document: id, alias, path, or a unique slug.
pub fn resolve(s: &Session, text: &str) -> Result<usize> {
    let rel = repo_path(s, text);
    for candidate in [text.to_string(), rel] {
        match s.graph.resolve(&candidate) {
            Resolution::Doc { idx, .. } | Resolution::MissingAnchor { idx, .. } => return Ok(idx),
            Resolution::Ambiguous(list) => {
                let ids: Vec<String> = list.iter().map(|&i| s.corpus.docs[i].id.to_string()).collect();
                bail!("'{text}' is ambiguous: {}", ids.join(", "));
            }
            Resolution::Missing => {}
        }
    }
    let by_slug: Vec<usize> = (0..s.corpus.docs.len()).filter(|&i| s.corpus.docs[i].slug == text).collect();
    match by_slug.as_slice() {
        [i] => Ok(*i),
        [] => {
            // A partial id (a type, owner or timestamp prefix) gets the ids it could mean.
            let near: Vec<String> = s
                .corpus
                .docs
                .iter()
                .map(|d| d.id.to_string())
                .filter(|id| {
                    // A bare timestamp or slug part matches after the owner as well.
                    let local = id.rsplit('/').next().unwrap_or(id);
                    text.len() >= 3 && (id.starts_with(text) || local.starts_with(text))
                })
                .collect();
            match near.len() {
                0 if text.contains('/') && !text.contains(':') => bail!("'{text}' names no document"),
                0 => {
                    // Words of a title or slug find it where an id prefix does not.
                    let words = if text.chars().all(|c| c.is_ascii_alphanumeric() || "-_:/.".contains(c)) {
                        text.to_string()
                    } else {
                        format!("'{}'", text.replace('\'', "'\\''"))
                    };
                    bail!("'{text}' names no document; next: spec find {words}")
                }
                1 => bail!("'{text}' names no document; 1 id starts with it: {0}; next: spec show {0}", near[0]),
                n => bail!(
                    "'{text}' names no document; {n} id(s) start with it: {}{}; next: spec show {}",
                    near.iter().take(5).cloned().collect::<Vec<_>>().join(", "),
                    if n > 5 { ", ..." } else { "" },
                    near[0]
                ),
            }
        }
        many => bail!(
            "'{text}' is ambiguous: {}",
            many.iter().map(|&i| s.corpus.docs[i].id.to_string()).collect::<Vec<_>>().join(", ")
        ),
    }
}

/// A document to put in an example command: of the preferred type if there is one, else the
/// most connected one, so the example shows something.
pub fn example_doc(s: &Session, prefer_ty: Option<&str>) -> Option<String> {
    let docs = &s.corpus.docs;
    if let Some(ty) = prefer_ty
        && let Some(d) = docs.iter().find(|d| d.id.ty == ty)
    {
        return Some(d.id.to_string());
    }
    let degree = |i: usize| s.graph.edges.iter().filter(|e| e.from == i || e.to == i).count();
    (0..docs.len()).max_by_key(|&i| (degree(i), std::cmp::Reverse(i))).map(|i| docs[i].id.to_string())
}

/// The same, opened from a command line that clap refused: honours -C/--dir and --no-local.
pub fn example_doc_for(argv: &[String], prefer_ty: Option<&str>) -> Option<String> {
    let mut dir = None;
    let mut it = argv.iter();
    while let Some(a) = it.next() {
        if a == "-C" || a == "--dir" {
            dir = it.next().map(PathBuf::from);
        } else if let Some(d) = a.strip_prefix("--dir=") {
            dir = Some(PathBuf::from(d));
        }
    }
    let g = Global {
        json: false,
        jsonl: false,
        limit: 30,
        offset: 0,
        max_bytes: None,
        dir,
        no_local: argv.iter().any(|a| a == "--no-local"),
    };
    let s = open(&g).ok()?;
    example_doc(&s, prefer_ty)
}
