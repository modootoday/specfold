//! `spec attach`.

use crate::cli::args::{global::Global, write::AttachArgs};
use crate::commands::outbound;
use crate::output::{Code, Output};
use crate::session;
use anyhow::Result;
use std::io::Read;

/// How much of one file the upload guard reads.
const SCAN_LIMIT: u64 = 64 * 1024 * 1024;
use serde_json::to_value;
use specfold_core::files::attach;

pub fn run(g: &Global, a: &AttachArgs) -> Result<Output> {
    let s = session::open(g)?;
    let idx = session::resolve(&s, &a.doc)?;
    let d = &s.corpus.docs[idx];
    let place = if a.store {
        attach::Where::Store
    } else if a.repo {
        attach::Where::Repo
    } else {
        attach::Where::Auto
    };
    let plan = attach::plan(&s.corpus, d, &a.files, &a.into, place)?;
    let mut o = Output::new("attach").field("id", d.id.to_string());
    if let Some((from, to)) = &plan.to_bundle {
        o = o.head(format!("{from} becomes a bundle: {to}"));
    }
    for f in &plan.files {
        o = o.item(to_value(f)?, format!("{:<5} {:>10}  {}  <- {}", f.store, f.bytes, f.path, f.source));
    }
    let uploads: Vec<&attach::Planned> = plan.files.iter().filter(|f| f.store == "store").collect();
    if !uploads.is_empty() {
        let mut blobs = vec![];
        for f in &uploads {
            let mut bytes = vec![];
            std::fs::File::open(&f.source)?.take(SCAN_LIMIT).read_to_end(&mut bytes)?;
            if f.bytes > SCAN_LIMIT {
                o = o.head(format!("{}: only the first 64 MiB was read by the upload guard", f.source));
            }
            blobs.push((f.path.clone(), bytes));
        }
        let gate = outbound::gate(&s.corpus.cfg, specfold_core::guards::reads_content, &blobs, &a.allow_hit)?;
        o = o.field("guard", gate.json());
        for l in gate.lines() {
            o = o.head(l);
        }
        if gate.refuses() {
            let refusing = gate.refusing();
            let files: Vec<String> = a.files.iter().map(|p| p.display().to_string()).collect();
            let allow: Vec<String> = refusing.iter().map(|g| format!("--allow-hit {g}")).collect();
            return Ok(o
                .code(Code::Findings)
                .foot(format!("nothing uploaded: guard {} refuses a file bound for the store", refusing.join(", ")))
                .next(format!(
                    "spec attach {} {}{} --write {}",
                    a.doc,
                    files.join(" "),
                    if a.store { " --store" } else { "" },
                    allow.join(" ")
                )));
        }
    }
    if a.write {
        attach::apply(&s.corpus, d, &plan)?;
        return Ok(o.foot("attached").next(format!("spec files {}", d.id)));
    }
    let files: Vec<String> = a.files.iter().map(|p| p.display().to_string()).collect();
    Ok(o.foot("preview only; nothing was written").next(format!("spec attach {} {} --write", a.doc, files.join(" "))))
}
