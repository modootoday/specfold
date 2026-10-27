//! `spec find`.

use crate::cli::args::{global::Global, read::FindArgs};
use crate::output::Output;
use crate::session;
use anyhow::Result;
use serde_json::to_value;
use specfold_core::search::{self, Query};

pub fn run(g: &Global, a: &FindArgs) -> Result<Output> {
    let s = session::open(g)?;
    let q = Query {
        words: a.words.clone(),
        types: a.types.clone(),
        owners: a.owners.clone(),
        statuses: a.statuses.clone(),
        tags: a.tags.clone(),
        all: a.all,
        text: a.text,
    };
    let hits = search::find(&s.corpus, &q);
    let mut o = Output::new("find");
    for h in &hits {
        let mut text = format!("{:<36} {:<10} {}", h.id, h.status, h.title);
        if let Some(e) = &h.excerpt {
            text.push_str(&format!("\n    {e}"));
        }
        o = o.item(to_value(h)?, text);
    }
    Ok(match hits.first() {
        Some(h) => o.next(format!("spec show {}", h.id)),
        None if !a.all => o.foot("nothing current matches").next("spec find --all ..."),
        None => o.foot("nothing matches"),
    })
}
