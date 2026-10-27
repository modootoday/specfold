//! `spec guard`.

use crate::cli::args::{global::Global, verify::GuardArgs};
use crate::output::{Code, Output, findings};
use crate::session;
use anyhow::Result;
use serde_json::json;
use specfold_core::guards::{self, GuardRange};

pub fn run(g: &Global, a: &GuardArgs) -> Result<Output> {
    let cfg = session::config(g)?;
    if a.write_baseline {
        let mut o = Output::new("guard");
        for (name, rel, hits) in guards::write_baselines(&cfg, &a.names)? {
            o = o
                .item(json!({"guard": name, "baseline": rel, "hits": hits}), format!("{name}: {hits} hit(s) in {rel}"));
        }
        return Ok(o.next("git add <baseline files>"));
    }
    let range = if a.staged {
        Some(GuardRange::Staged)
    } else if a.all {
        Some(GuardRange::All)
    } else {
        a.range.as_deref().map(GuardRange::parse)
    };
    let list = guards::run(&cfg, &a.names, range.as_ref())?;
    let (errors, warnings) = findings::counts(&list);
    let o = findings::push_all(Output::new("guard").field("errors", errors).field("warnings", warnings), &list);
    let names = if a.names.is_empty() {
        cfg.raw.guards.iter().map(|g| g.name.clone()).collect::<Vec<_>>().join(", ")
    } else {
        a.names.join(", ")
    };
    let o = o.foot(format!("guards {names}: {errors} error(s), {warnings} warning(s)"));
    Ok(if errors > 0 { o.code(Code::Findings).next("spec explain U1") } else { o })
}
