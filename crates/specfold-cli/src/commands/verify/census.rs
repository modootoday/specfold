//! `spec census`.

use crate::cli::args::{global::Global, verify::CensusArgs};
use crate::output::{Code, Output};
use crate::session;
use anyhow::Result;
use serde_json::to_value;
use specfold_core::census;

pub fn run(g: &Global, a: &CensusArgs) -> Result<Output> {
    let s = session::open(g)?;
    let c = census::take(&s.corpus, &s.graph);
    let mut o = Output::new("census").unpaged();
    if let serde_json::Value::Object(m) = to_value(&c)? {
        o.summary.extend(m);
    }
    if a.legacy {
        let gone = c.legacy_paths == 0 && c.strays == 0 && c.links_via_alias == 0 && c.path_cites == 0;
        o = o.head(format!(
            "legacy paths {} · strays {} · alias references {} · path citations {}",
            c.legacy_paths, c.strays, c.links_via_alias, c.path_cites
        ));
        return Ok(if gone { o.foot("the old layout is gone") } else { o.code(Code::Findings).next("spec check") });
    }
    o = o.head(format!("{} documents in {} spec directories · {} strays", c.docs, c.owners_with_docs, c.strays));
    o = o.head(format!(
        "references {} (broken {}, via alias {}) · path citations {}",
        c.links, c.links_broken, c.links_via_alias, c.path_cites
    ));
    o = o.head(format!(
        "attachments {} ({} bytes) · aliases {} · legacy paths {}",
        c.attachments, c.attachment_bytes, c.aliases, c.legacy_paths
    ));
    for (t, n) in &c.by_type {
        o = o.foot(format!("  {t:<10} {n:>6}"));
    }
    Ok(o.next("spec audit"))
}
