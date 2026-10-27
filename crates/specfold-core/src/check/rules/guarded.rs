//! U1: the project's guards pass for the range being checked.

use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::check::scope::Range;
use crate::guards::{self, GuardRange};

pub struct GuardsPass;

impl Rule for GuardsPass {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "U1",
            title: "guards pass",
            explain: "Guards are named checks in spec.toml over any file and over commit messages: denied terms (inline, from a private file, or from an environment variable), regexes, file patterns, size limits and built-in secret formats. Terms are masked in output. Run one with `spec guard <name>`.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        if ctx.corpus.cfg.raw.guards.is_empty() {
            return;
        }
        let range = match &ctx.scope.range {
            Range::All => GuardRange::All,
            Range::Staged => GuardRange::Staged,
            Range::Changed(base) => GuardRange::Push(Some(format!("{base}..HEAD"))),
        };
        match guards::run(&ctx.corpus.cfg, &[], Some(&range)) {
            Ok(f) => out.extend(f),
            Err(e) => out.push(Finding::error("U1", "spec.toml", 1, e.to_string())),
        }
    }
}
