//! `spec sync visibility`: measures each submodule's visibility on GitHub and writes
//! `[public] submodules_manifest`; previews until --write.

use super::preview;
use crate::cli::args::{global::Global, write::SyncArgs};
use crate::output::{Code, Output};
use crate::session;
use anyhow::Result;
use serde_json::json;
use specfold_core::authoring::Plan;
use specfold_core::workspace::visibility::{self, ForgeError, GitHub, Measured};

const RERUN: &str = "spec sync visibility --write";

pub fn run(g: &Global, a: &SyncArgs) -> Result<Output> {
    if a.prune || a.force {
        let msg = "--prune and --force apply to spec sync assets; spec sync visibility takes neither";
        return Ok(Output::error("sync", Code::Usage, msg, Some("spec sync visibility".into())));
    }
    let cfg = session::config(g)?;
    let Some(rel) = cfg.raw.public.submodules_manifest.clone() else {
        return Ok(Output::new("sync")
            .field("written", false)
            .head("[public] submodules_manifest is not set: there is no visibility manifest to write")
            .next("spec explain config   # then set [public] submodules_manifest and run spec sync visibility"));
    };
    let now = specfold_core::model::stamp::now_value();
    let m = match visibility::measure(&cfg.spec_home, &rel, &GitHub, &now) {
        Ok(m) => m,
        Err(e) => return Ok(refused(&rel, &e, a.write)),
    };
    let o = summary(&m);
    if !m.changed {
        return Ok(o
            .field("written", false)
            .head(format!("{rel} matches .gitmodules and GitHub: nothing to write"))
            .next("spec check"));
    }
    let mut plan = Plan::default();
    plan.write(rel.clone(), m.text.clone(), !m.exists);
    let o = preview::finish(o, &plan, &cfg.spec_home, a.write, RERUN)?;
    Ok(if a.write { o.next(format!("git add {}", specfold_core::suggest::shell::word(&rel))) } else { o })
}

/// The measurement: every submodule, and what changed against the file on disk.
fn summary(m: &Measured) -> Output {
    let subs: Vec<_> = m
        .after
        .submodules
        .iter()
        .map(|(path, e)| json!({"path": path, "url": e.url, "visibility": e.visibility}))
        .collect();
    let mut o = Output::new("sync")
        .field("manifest", m.rel.clone())
        .field("changed", m.changed)
        .field("submodules", json!(subs))
        .field("added", json!(m.added))
        .field("removed", json!(m.removed))
        .field("updated", json!(m.updated));
    let public = m.after.submodules.iter().filter(|(_, e)| e.is_public()).count();
    o = o.head(format!("{} submodule(s) measured on GitHub, {public} public", m.after.submodules.len()));
    for (label, list) in [("added", &m.added), ("removed", &m.removed), ("updated", &m.updated)] {
        if !list.is_empty() {
            o = o.head(format!("{label}: {}", list.join(", ")));
        }
    }
    o
}

/// Nothing is written when a URL is off GitHub or GitHub cannot be asked: a manifest that
/// misses a submodule would let V1 pass a public one.
fn refused(rel: &str, e: &ForgeError, write: bool) -> Output {
    let (code, msg, next) = match e {
        ForgeError::Unsupported(m) => (
            if write { Code::Refused } else { Code::Findings },
            format!("{m}; {rel} was not written"),
            "spec explain V1",
        ),
        ForgeError::Unreachable(m) => (
            Code::Env,
            format!("{m}; {rel} was not written, as visibility cannot be measured offline"),
            "gh auth status   # then spec sync visibility",
        ),
        ForgeError::Failed(m) => (Code::Env, format!("{m}; {rel} was not written"), "gh auth status"),
    };
    Output::error("sync", code, &msg, Some(next.to_string()))
}
