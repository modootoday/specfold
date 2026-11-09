//! Which documents a run looks at: all of them, or the ones touched by a change and the
//! documents that point at them.

use crate::corpus::Corpus;
use crate::corpus::graph::Graph;
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
    /// What one commit changed (`<sha>^..<sha>`), judged on the files as they are now (C3).
    Commit(String),
    /// These repository files, documents or not, as they are on disk or in an overlay
    /// (`spec check --path`); nothing is compared with a revision.
    Paths(Vec<String>),
}

#[derive(Debug, Clone)]
pub struct Scope {
    pub range: Range,
    /// Changed repository paths; empty for `All`.
    pub paths: BTreeSet<String>,
    /// The part of `paths` inside submodules whose gitlink the change moves; the outer index
    /// cannot show them, so a staged run reads them from the submodule's checkout.
    pub in_submodules: BTreeSet<String>,
    /// Indices of documents whose own files changed.
    pub touched: BTreeSet<usize>,
    /// `touched` plus documents that link to them.
    pub with_neighbours: BTreeSet<usize>,
    /// The revision to compare against for transition checks.
    pub base: Option<String>,
    /// Paths the change deletes or renames away; they are not in `paths`.
    pub gone: BTreeSet<String>,
    /// Moved gitlinks whose submodule objects are missing; they stand for the whole directory.
    pub submodules_missing: Vec<String>,
    /// Gitlinks the change adds (a split, when outer files moved into one) or bumps, each read
    /// as one event: files moved unchanged into a new gitlink are in neither `paths` nor `gone`.
    pub gitlinks: Vec<vcs::GitlinkMove>,
}

/// The slug a document file carries: `sot/cart.sot.md` and `sot/cart/index.md` give `cart`.
fn slug_of(p: &str) -> Option<&str> {
    let mut parts = p.rsplit('/');
    let file = parts.next()?;
    let stem = file.split('.').next()?;
    if stem == "index" || stem == "README" { parts.next() } else { Some(stem) }
}

/// Documents with a relation or link that now resolves to nothing and names a deleted slug or path.
fn dangling(c: &Corpus, g: &Graph, slugs: &BTreeSet<&str>) -> Vec<usize> {
    use crate::corpus::graph::{DOC_RELATIONS, Resolution};
    let names = |t: &str| {
        let t = t.split('#').next().unwrap_or(t).trim_end_matches('/');
        let last = t.rsplit(['/', ':']).next().unwrap_or(t);
        slugs.contains(last) || slug_of(t).is_some_and(|s| slugs.contains(s))
    };
    let missing = |t: &str| names(t) && matches!(g.resolve(t), Resolution::Missing);
    let mut out = vec![];
    for (i, d) in c.docs.iter().enumerate() {
        let rel = d.rel().into_iter().filter(|(k, _)| DOC_RELATIONS.contains(&k.as_str())).flat_map(|(_, ts)| ts);
        let mut targets = rel.chain(d.links.iter().map(|l| l.item.clone()));
        if targets.any(|t| missing(&t)) {
            out.push(i);
        }
    }
    out
}

impl Scope {
    pub fn build(c: &Corpus, g: &Graph, range: Range) -> Result<Self> {
        let root = &c.cfg.spec_home;
        let (mut paths, sub, base): (BTreeSet<String>, vcs::Expansion, _) = match &range {
            Range::All => (BTreeSet::new(), vcs::Expansion::default(), None),
            Range::Staged => {
                let p = vcs::staged(root).ok_or_else(|| anyhow!("--staged needs a git repository"))?;
                let sub = vcs::expand_submodules(root, vcs::Side::Staged);
                (p.into_iter().collect(), sub, Some("HEAD".to_string()))
            }
            Range::Changed(base) => {
                let p = vcs::changed_since(root, base).ok_or_else(|| anyhow!("cannot diff against {base}"))?;
                let sub = vcs::expand_submodules(root, vcs::Side::Since(base));
                (p.into_iter().collect(), sub, Some(base.clone()))
            }
            Range::Commit(sha) => {
                let r = vcs::commit_diff(root, sha);
                let p = vcs::changed_in_range(root, &r)
                    .ok_or_else(|| anyhow!("cannot read commit {sha}; next: git log --oneline"))?;
                (p.into_iter().collect(), vcs::Expansion::default(), Some(vcs::parent_or_empty(root, sha)))
            }
            Range::Paths(named) => (named.iter().cloned().collect(), vcs::Expansion::default(), None),
        };
        // C1: a moved pointer whose inside was read is not itself a change.
        for g in &sub.expanded {
            paths.remove(g);
        }
        let in_submodules: BTreeSet<String> = sub.files.into_iter().collect();
        paths.extend(in_submodules.iter().cloned());
        let mut gone: BTreeSet<String> = match &range {
            Range::All | Range::Paths(_) => BTreeSet::new(),
            Range::Staged => vcs::gone(root, None).unwrap_or_default().into_iter().collect(),
            Range::Changed(base) => vcs::gone(root, Some(base)).unwrap_or_default().into_iter().collect(),
            Range::Commit(sha) => {
                vcs::gone(root, Some(&vcs::commit_diff(root, sha))).unwrap_or_default().into_iter().collect()
            }
        };
        // A path that now lies inside a gitlink the change adds was moved there, not removed.
        for m in sub.moves.iter().filter(|m| m.old.is_none()) {
            for p in m.moved.iter().chain(&m.changed) {
                gone.remove(p);
            }
        }
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
        // N2: a deleted document is no longer in the corpus, so its citers cannot be found
        // through edges; one pass over every link finds the ones that now point nowhere.
        let slugs: BTreeSet<&str> = gone.iter().filter(|p| p.ends_with(".md")).filter_map(|p| slug_of(p)).collect();
        if !slugs.is_empty() {
            with_neighbours.extend(dangling(c, g, &slugs));
        }
        Ok(Scope {
            range,
            paths,
            in_submodules,
            touched,
            with_neighbours,
            base,
            gone,
            submodules_missing: sub.missing,
            gitlinks: sub.moves,
        })
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
