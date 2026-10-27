//! Which documents a command run reaches, and which of their `verify` entries are commands.

use crate::check::context::Ctx;
use crate::graph::query;
use crate::model::Doc;
use crate::workspace::owners::matcher;

/// The marker for a rule that has no runnable check and is verified by people.
const MANUAL: &str = "manual";

pub fn verify_commands(d: &Doc) -> Vec<String> {
    d.list_field("verify").into_iter().filter(|c| !c.trim().eq_ignore_ascii_case(MANUAL)).collect()
}

fn is_gitlink(ctx: &Ctx, path: &str) -> bool {
    ctx.corpus.cfg.root.join(path).join(".git").exists()
}

/// Every document for a full run or when a path in `full_run_on` changed; otherwise those
/// governing a changed path (a changed submodule reaches globs inside it), those whose command
/// names a changed file, and those edited themselves.
pub fn selected(ctx: &Ctx) -> Vec<usize> {
    let all = || (0..ctx.corpus.docs.len()).collect();
    if ctx.scope.is_all() {
        return all();
    }
    let full: Vec<_> = ctx.corpus.cfg.raw.verify.full_run_on.iter().filter_map(|p| matcher(p)).collect();
    if ctx.scope.paths.iter().any(|p| full.iter().any(|m| m.is_match(p))) {
        return all();
    }
    let mut out: Vec<usize> =
        ctx.scope.paths.iter().flat_map(|p| query::governing(ctx.corpus, p).into_iter().map(|g| g.idx)).collect();
    for (i, d) in ctx.corpus.docs.iter().enumerate() {
        let commands = verify_commands(d);
        let governs = d.rel().get("governs").cloned().unwrap_or_default();
        let reached = ctx.scope.paths.iter().any(|p| {
            commands.iter().any(|c| c.contains(p.as_str()))
                || (is_gitlink(ctx, p) && governs.iter().any(|g| g.starts_with(&format!("{p}/"))))
        });
        if reached {
            out.push(i);
        }
    }
    out.extend(ctx.scope.touched.iter().copied());
    out.sort();
    out.dedup();
    out
}
