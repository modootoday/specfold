//! `spec sync assets`: brings asset copies in line with their `copy_of` source; previews until --write.

use super::preview;
use crate::cli::args::{global::Global, write::SyncArgs};
use crate::output::Output;
use crate::session;
use anyhow::Result;
use serde_json::json;
use specfold_core::assets::sync;
use specfold_core::authoring::Change;

pub fn run(g: &Global, a: &SyncArgs) -> Result<Output> {
    let s = session::open(g)?;
    let cfg = &s.corpus.cfg;
    let (pairs, mut plan, unowned) = sync::plan(cfg, a.prune)?;
    // C100: a pruned file may never have been committed, so git cannot bring it back; it moves
    // to a trash directory specfold owns, and the undo copies it back.
    let bin = format!("{}/{}", preview::TRASH, specfold_core::model::stamp::now_stamp());
    for c in plan.changes.iter_mut() {
        if let Change::Remove { path } = c {
            *c = Change::Move { from: path.clone(), to: format!("{bin}/{path}") };
        }
    }
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
    let removes = plan.changes.iter().any(|c| matches!(c, Change::Move { .. }));
    if a.write && removes && !a.yes {
        let o = o
            .head("--prune deletes files: nothing was written; add --yes to confirm")
            .code(crate::output::Code::Refused);
        return preview::finish(o, &plan, &cfg.root, false, "spec sync assets --prune --write --yes");
    }
    // CLI-4: overwriting a file that is not a past version of its source needs --force.
    if !unowned.is_empty() && !a.force {
        o = o.field("unowned", json!(unowned)).head(format!(
            "{} copy file(s) differ from every version of their source and would be overwritten{}: {}; add --force to overwrite",
            unowned.len(),
            if a.write { ", so nothing was written" } else { "" },
            unowned.join(", ")
        ));
        if a.write {
            let mut rerun = String::from("spec sync assets --write --force");
            if a.prune {
                rerun.push_str(" --prune --yes");
            }
            return preview::finish(o.code(crate::output::Code::Refused), &plan, &cfg.root, false, &rerun);
        }
    }
    let rerun = match (a.prune, !unowned.is_empty()) {
        (true, true) => "spec sync assets --prune --write --yes --force",
        (true, false) => "spec sync assets --prune --write --yes",
        (false, true) => "spec sync assets --write --force",
        (false, false) => "spec sync assets --write",
    };
    let (o, go) = preview::bound(o, preview::files(&plan), cfg.raw.write.max_files, a.write, a.yes);
    let rerun =
        if o.summary.contains_key("bounded") && !a.prune { format!("{rerun} --yes") } else { rerun.to_string() };
    if go {
        // The trash never shows up in git status or gets committed.
        let ignore = cfg.root.join(".specfold/.gitignore");
        if removes && !ignore.exists() {
            std::fs::create_dir_all(cfg.root.join(".specfold"))?;
            std::fs::write(ignore, "*\n")?;
        }
    }
    let o = preview::finish(o, &plan, &cfg.root, go, &rerun)?;
    Ok(if go {
        let o = if removes { o.foot(format!("pruned files are kept in {bin}/ until you delete it")) } else { o };
        o.next("git status --short")
    } else {
        o
    })
}
