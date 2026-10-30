//! `spec sync assets`: brings asset copies in line with their `copy_of` source; previews until --write.

use super::preview;
use crate::cli::args::{global::Global, write::SyncArgs};
use crate::output::Output;
use crate::session;
use anyhow::Result;
use serde_json::json;
use specfold_core::assets::sync;

pub fn run(g: &Global, a: &SyncArgs) -> Result<Output> {
    let s = session::open(g)?;
    let cfg = &s.corpus.cfg;
    let (pairs, plan) = sync::plan(cfg, a.prune)?;
    let mut o = Output::new("sync").field("roots", pairs.len()).field("changes", plan.changes.len());
    if pairs.is_empty() {
        let found: Vec<&str> =
            [".agents", ".claude", ".cursor", ".github/copilot-instructions.md", "AGENTS.md", "CLAUDE.md"]
                .into_iter()
                .filter(|p| cfg.root.join(p).exists())
                .collect();
        o = o.field("agent_files", json!(found));
        if found.is_empty() {
            return Ok(o
                .head("no agent files found (.agents, .claude, .cursor, AGENTS.md, CLAUDE.md): nothing to sync")
                .next("spec new skill <name>   # or add agent files, then declare a copy in spec.toml"));
        }
        return Ok(o.head(format!("agent files found ({}), but no copy is declared", found.join(", "))).next("declare a copy in spec.toml: [[assets]] kind = \"skill\", path = \".claude/skills\", copy_of = \".agents/skills\""));
    }
    o = o.field("pairs", json!(pairs.iter().map(|p| json!({"source": p.source, "copy": p.copy})).collect::<Vec<_>>()));
    for p in &pairs {
        o = o.head(format!("{} <- {}", p.copy, p.source));
    }
    if plan.changes.is_empty() {
        return Ok(o.field("written", false).foot("every copy matches its source").next("spec audit --kind drift"));
    }
    // Deleting files is destructive: --prune applies only with --yes as well (CLI-3).
    let removes = plan.changes.iter().any(|c| matches!(c, specfold_core::authoring::Change::Remove { .. }));
    if a.write && removes && !a.yes {
        let o = o.head("--prune deletes files: nothing was written; add --yes to confirm");
        return preview::finish(o, &plan, &cfg.root, false, "spec sync assets --prune --write --yes");
    }
    let rerun = if a.prune { "spec sync assets --prune --write --yes" } else { "spec sync assets --write" };
    let (o, go) = preview::bound(o, preview::files(&plan), cfg.raw.write.max_files, a.write, a.yes);
    let rerun =
        if o.summary.contains_key("bounded") && !a.prune { format!("{rerun} --yes") } else { rerun.to_string() };
    preview::finish(o, &plan, &cfg.root, go, &rerun)
}
