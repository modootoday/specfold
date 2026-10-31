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

use super::replace::{self, Clash};

/// The rerun line with the same document, files, place and folder.
fn again(a: &AttachArgs, flags: &str) -> String {
    let files: Vec<String> = a.files.iter().map(|p| specfold_core::shell::word(p.display())).collect();
    let into =
        if a.into.is_empty() { String::new() } else { format!(" --into {}", specfold_core::shell::quote(&a.into)) };
    format!(
        "spec attach {} {}{}{into}{flags}",
        specfold_core::shell::word(&a.doc),
        files.join(" "),
        if a.store { " --store" } else { "" }
    )
}

/// C202/C203: a write that would replace other bytes needs --yes, and one replacing a file the
/// document does not list needs --force too (as `spec fetch`). True when the output is final.
fn clash_gate(o: Output, a: &AttachArgs, clashes: &[Clash]) -> (Output, bool) {
    if clashes.is_empty() {
        return (o, false);
    }
    let names: Vec<&str> = clashes.iter().map(|c| c.path.as_str()).collect();
    let o = o.head(format!("{} already exist(s) with other content and would be replaced", names.join(", ")));
    let unlisted: Vec<&str> = clashes.iter().filter(|c| !c.registered).map(|c| c.path.as_str()).collect();
    if !a.write {
        return (o, false);
    }
    if !unlisted.is_empty() && !a.force {
        let o = o
            .code(Code::Usage)
            .head(format!("{} is not in the document's files, so spec did not put it there", unlisted.join(", ")))
            .foot("nothing was written; move the file aside, or add --force to replace it (the old copy goes to the trash)")
            .next(again(a, " --write --yes --force"));
        return (o, true);
    }
    if !a.yes {
        let force = if unlisted.is_empty() { "" } else { " --force" };
        let o = o
            .code(Code::Refused)
            .foot("nothing was written; add --yes to replace (the old copy goes to the trash)")
            .next(again(a, &format!(" --write --yes{force}")));
        return (o, true);
    }
    (o, false)
}

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
    let files: Vec<std::path::PathBuf> = a.files.iter().map(|p| session::arg_path(g, p)).collect();
    let plan = attach::plan(&s.corpus, d, &files, &a.into, place)?;
    // C143: every next keeps --into, so it attaches where the preview showed.
    let into =
        if a.into.is_empty() { String::new() } else { format!(" --into {}", specfold_core::shell::quote(&a.into)) };
    let mut o = Output::new("attach").field("id", d.id.to_string());
    if let Some((from, to)) = &plan.to_bundle {
        o = o.head(format!("{from} becomes a bundle: {to}"));
    }
    let clashes = replace::clashes(&s.corpus.cfg.root, d, &plan);
    for f in &plan.files {
        let clash = clashes.iter().find(|c| c.path.ends_with(&format!("/{}", f.path)));
        let mut item = to_value(f)?;
        item["replace"] = clash.is_some().into();
        let mark = if clash.is_some() { "  (replace: other content there now)" } else { "" };
        o = o.item(item, format!("{:<5} {:>10}  {}  <- {}{mark}", f.store, f.bytes, f.path, f.source));
    }
    let (o2, stop) = clash_gate(o, a, &clashes);
    o = o2;
    if stop {
        return Ok(o);
    }
    // Every planned file gets the built-in secrets check: a hit refuses an upload to the store
    // and is a warning for a file that stays in the repository; configured guards gate uploads.
    if !plan.files.is_empty() {
        let (mut uploads, mut kept) = (vec![], vec![]);
        for f in &plan.files {
            let mut bytes = vec![];
            std::fs::File::open(&f.source)?.take(SCAN_LIMIT).read_to_end(&mut bytes)?;
            if f.bytes > SCAN_LIMIT {
                o = o.head(format!("{}: only the first 64 MiB was read by the upload guard", f.source));
            }
            if f.store == "store" { &mut uploads } else { &mut kept }.push((f.path.clone(), bytes));
        }
        let mut gate = outbound::gate(&s.corpus.cfg, specfold_core::guards::reads_content, &uploads, &a.allow_hit)?;
        gate.add_builtin(&uploads, &a.allow_hit, false);
        gate.add_builtin(&kept, &a.allow_hit, true);
        if !gate.hits.is_empty() || !uploads.is_empty() {
            o = o.field("guard", gate.json());
        }
        for l in gate.lines() {
            o = o.head(l);
        }
        if gate.refuses() {
            let refusing = gate.refusing();
            let files: Vec<String> = a.files.iter().map(|p| specfold_core::shell::word(p.display())).collect();
            let allow: Vec<String> = refusing.iter().map(|g| format!("--allow-hit {g}")).collect();
            return Ok(o
                .code(Code::Findings)
                .foot(format!("nothing uploaded: guard {} refuses a file bound for the store", refusing.join(", ")))
                .next(format!(
                    "spec attach {} {}{}{into} --write --yes {}",
                    specfold_core::shell::word(&a.doc),
                    files.join(" "),
                    if a.store { " --store" } else { "" },
                    allow.join(" ")
                )));
        }
    }
    // Sending a file to the store leaves the repository: it needs --yes as well (CLI-3).
    let uploads = plan.files.iter().any(|f| f.store == "store");
    let force = if clashes.iter().any(|c| !c.registered) { " --force" } else { "" };
    if a.write && uploads && !a.yes {
        return Ok(o
            .code(crate::output::Code::Refused)
            .head("a file goes to the store, outside the repository: nothing was written; add --yes to confirm")
            .foot("preview only; nothing was written")
            .next(again(a, &format!(" --write --yes{force}"))));
    }
    if a.write {
        let undo = replace::keep(&s.corpus.cfg.root, &d.path, &clashes)?;
        attach::apply(&s.corpus, d, &plan)?;
        if let Some(undo) = undo {
            o = o
                .field("undo", undo.clone())
                .foot(format!("undo (puts the replaced files and the document back): {undo}"));
        }
        return Ok(o.foot("attached").next(format!("spec files {}", specfold_core::shell::word(&d.id))));
    }
    let confirm = if uploads || !clashes.is_empty() { " --yes" } else { "" };
    Ok(o.foot("preview only; nothing was written").next(again(a, &format!(" --write{confirm}{force}"))))
}
