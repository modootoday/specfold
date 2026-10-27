//! Which documents a run looks at: all of them, or the ones touched by a change and the
//! documents that point at them.

use crate::corpus::Corpus;
use crate::graph::Graph;
use crate::workspace::vcs;
use anyhow::{Result, anyhow};
use std::collections::BTreeSet;

#[derive(Debug, Clone)]
pub enum Range {
    /// Everything.
    All,
    /// The index (pre-commit).
    Staged,
    /// Working tree and untracked files against a base revision (pre-push).
    Changed(String),
}

#[derive(Debug, Clone)]
pub struct Scope {
    pub range: Range,
    /// Changed repository paths; empty for `All`.
    pub paths: BTreeSet<String>,
    /// Indices of documents whose own files changed.
    pub touched: BTreeSet<usize>,
    /// `touched` plus documents that link to them.
    pub with_neighbours: BTreeSet<usize>,
    /// The revision to compare against for transition checks.
    pub base: Option<String>,
}

impl Scope {
    pub fn build(c: &Corpus, g: &Graph, range: Range) -> Result<Self> {
        let (paths, base) = match &range {
            Range::All => (BTreeSet::new(), None),
            Range::Staged => {
                let p = vcs::staged(&c.cfg.root).ok_or_else(|| anyhow!("--staged needs a git repository"))?;
                (p.into_iter().collect(), Some("HEAD".to_string()))
            }
            Range::Changed(base) => {
                let p = vcs::changed_since(&c.cfg.root, base).ok_or_else(|| anyhow!("cannot diff against {base}"))?;
                (p.into_iter().collect(), Some(base.clone()))
            }
        };
        let mut touched = BTreeSet::new();
        if !matches!(range, Range::All) {
            for (i, d) in c.docs.iter().enumerate() {
                let own = std::iter::once(&d.path).chain(&d.chapters).chain(&d.bundle_files);
                if own.into_iter().any(|p| paths.contains(p)) {
                    touched.insert(i);
                }
            }
        }
        let mut with_neighbours = touched.clone();
        for &i in &touched {
            for e in g.edges_to(i) {
                with_neighbours.insert(e.from);
            }
        }
        Ok(Scope { range, paths, touched, with_neighbours, base })
    }

    pub fn is_all(&self) -> bool {
        matches!(self.range, Range::All)
    }

    /// Whether checks about document `i` itself run.
    pub fn covers(&self, i: usize) -> bool {
        self.is_all() || self.touched.contains(&i)
    }

    /// Whether checks about links from document `i` run (they may break when a target moves).
    pub fn covers_links(&self, i: usize) -> bool {
        self.is_all() || self.with_neighbours.contains(&i)
    }

    pub fn covers_path(&self, p: &str) -> bool {
        self.is_all() || self.paths.contains(p)
    }
}
