//! `spec log`: status and relation history from git.

use crate::cli::args::{global::Global, read::LogArgs};
use crate::output::Output;
use crate::session;
use anyhow::{Result, bail};
use serde_json::json;
use specfold_core::audit::history;

pub fn run(g: &Global, a: &LogArgs) -> Result<Output> {
    let s = session::open(g)?;
    let c = &s.corpus;
    let picks: Vec<usize> = match &a.doc {
        Some(r) => vec![session::resolve(&s, r)?],
        None if a.since.is_some() => {
            (0..c.docs.len()).filter(|&i| a.types.is_empty() || a.types.contains(&c.docs[i].id.ty)).collect()
        }
        None => bail!("give a document, or --since <YYYY-MM-DD>"),
    };
    let mut o = Output::new("log");
    for i in picks {
        let d = &c.docs[i];
        for e in history::events(&c.cfg.root, &d.path) {
            if a.since.as_deref().is_some_and(|since| e.date.as_str() < since) {
                continue;
            }
            let text = format!("{}  {}  {:<36} {:<40} {}", e.date, e.commit, d.id.to_string(), e.change, e.author);
            o = o.item(json!({"id": d.id.to_string(), "date": e.date, "commit": e.commit, "author": e.author, "change": e.change}), text);
        }
    }
    Ok(o.next("spec show <id> --rel"))
}
