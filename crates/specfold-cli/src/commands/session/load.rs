//! Opening a command's session: its location, configuration, corpus and graph.

use super::Location;
use crate::cli::args::global::Global;
use anyhow::{Result, bail};
use specfold_core::{Config, Corpus, Graph, LoadOptions};
use std::path::PathBuf;
use std::sync::Arc;

pub struct Session {
    pub corpus: Corpus,
    pub graph: Graph,
    /// Where the command runs, with this session's spec root.
    pub loc: Location,
}

/// The configuration. `[types.<name>]` tables that look like typos are warned about on standard
/// error only when a command asked for it (`spec check`, `-q` too); doctor lists them as
/// findings, and every other command stays quiet about them.
pub fn config(g: &Global) -> Result<Config> {
    let cfg = config_quiet(g)?;
    super::warnings::show_once(&cfg);
    Ok(cfg)
}

/// The location of the command `g` runs, read once per command (-C, the process directory).
pub fn location(g: &Global) -> Location {
    match &g.loc {
        Some(l) => l.without_config(),
        None => Location::at(g, g.dir.clone()),
    }
}

/// The location `execute` builds once per command, with the configuration it found.
pub fn discover(g: &Global, dir_as_typed: Option<PathBuf>) -> Arc<Location> {
    Arc::new(Location::at(g, dir_as_typed).discovered())
}

/// The configuration, found once per command: from the command's location when it holds one.
pub fn config_quiet(g: &Global) -> Result<Config> {
    if let Some(cfg) = g.loc.as_ref().and_then(|l| l.config()) {
        return Ok(cfg.clone());
    }
    Config::discover(&location(g).start()?)
}

pub fn open(g: &Global) -> Result<Session> {
    let cfg = config(g)?;
    let loc = location(g).rooted(&cfg);
    let corpus = Corpus::load(cfg, &LoadOptions { include_local: !g.no_local });
    let t = std::time::Instant::now();
    let graph = Graph::build(&corpus);
    specfold_core::trace::phase("graph", t);
    Ok(Session { corpus, graph, loc })
}

/// The session that holds `doc`: this repository, or for `<remote>:<id>` the remote repository
/// named in `[remotes]` (read-only), with the id left to resolve there and the remote's name.
pub fn open_for(g: &Global, doc: &str) -> Result<(Session, String, Option<String>)> {
    let cfg = config(g)?;
    if let Some((name, rest)) = specfold_core::workspace::remotes::split(&cfg, doc) {
        let dir = specfold_core::workspace::remotes::dir(&cfg, name).unwrap_or_default();
        if !dir.is_dir() {
            bail!(
                "the remote {name} is not on disk ({}); check it out there or fix [remotes.{name}] path",
                dir.display()
            );
        }
        let remote = Config::discover(&dir)?;
        let loc = location(g).for_remote(&remote.spec_home);
        let corpus = Corpus::load(remote, &LoadOptions { include_local: false });
        let graph = Graph::build(&corpus);
        return Ok((Session { corpus, graph, loc }, rest.to_string(), Some(name.to_string())));
    }
    let loc = location(g).rooted(&cfg);
    let corpus = Corpus::load(cfg, &LoadOptions { include_local: !g.no_local });
    let graph = Graph::build(&corpus);
    Ok((Session { corpus, graph, loc }, doc.to_string(), None))
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
        loc: None,
    };
    let s = open(&g).ok()?;
    super::resolve::example_doc(&s, prefer_ty)
}
