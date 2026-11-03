//! Everything a rule needs: the corpus, its graph, the scope of the run and the options.

use super::scope::Scope;
use crate::corpus::Corpus;
use crate::graph::Graph;
use crate::workspace::vcs;

#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Run sot `verify` and `generated_by` commands.
    pub run_commands: bool,
    /// Report broken citations in external roots (Claude memory, notes …).
    pub external: bool,
    /// Ask stores whether attachments exist (network, slow).
    pub online: bool,
    /// Run guards over the whole tree in a full run; staged and ranged runs always run them.
    pub guards: bool,
    /// The spec commands this version has, for checking commands named in agent assets; empty
    /// skips that check.
    pub commands: Vec<String>,
}

pub struct Ctx<'a> {
    pub corpus: &'a Corpus,
    pub graph: &'a Graph,
    pub scope: &'a Scope,
    pub opts: &'a Options,
}

impl Ctx<'_> {
    /// The content of a path at the base of the run (HEAD for staged runs).
    pub fn previous(&self, path: &str) -> Option<String> {
        let base = self.scope.base.as_deref()?;
        vcs::show(&self.corpus.cfg.spec_home, base, path)
    }

    /// The content being checked: the index for staged runs, the working tree otherwise.
    pub fn current(&self, path: &str) -> Option<String> {
        let staged = matches!(self.scope.range, super::scope::Range::Staged);
        if staged && !self.scope.in_submodules.contains(path) {
            return vcs::show_staged(&self.corpus.cfg.spec_home, path);
        }
        crate::corpus::overlay::read(&self.corpus.cfg.spec_home.join(path)).ok()
    }
}

/// The 1-based line of a top-level frontmatter key in a file, or 1.
pub fn key_line(ctx: &Ctx, path: &str, key: &str) -> usize {
    let Some(text) = ctx.current(path) else { return 1 };
    let prefix = format!("{key}:");
    text.lines().position(|l| l.starts_with(&prefix)).map(|i| i + 1).unwrap_or(1)
}
