//! `spec edit`.

use super::preview;
use crate::cli::args::{global::Global, write::EditArgs};
use crate::output::Output;
use crate::session;
use anyhow::{Context, Result};
use specfold_core::authoring::edit::{self, Edit, Mode};
use specfold_core::markdown::tables::RowChange;
use std::io::Read;

const SHOWN: usize = 40;

pub fn run(g: &Global, a: &EditArgs) -> Result<Output> {
    let s = session::open(g)?;
    let idx = session::resolve(&s, &a.doc)?;
    let cells_only = a.row.is_some() && !a.cell.is_empty() && a.text.is_none() && a.file.is_none();
    let text = match (&a.text, &a.file) {
        _ if cells_only => String::new(),
        (Some(t), _) => t.clone(),
        (None, Some(p)) if p.as_os_str() == "-" => {
            let mut t = String::new();
            std::io::stdin().read_to_string(&mut t).context("reading standard input")?;
            t
        }
        (None, Some(p)) => std::fs::read_to_string(p).with_context(|| format!("reading {}", p.display()))?,
        (None, None) => {
            use std::io::IsTerminal;
            if std::io::stdin().is_terminal() {
                anyhow::bail!("give --text or --file, or pipe the text in (spec edit ... < body.md)");
            }
            let mut t = String::new();
            std::io::stdin().read_to_string(&mut t).context("reading standard input")?;
            if t.is_empty() {
                anyhow::bail!("standard input was empty; give --text or --file, or pipe the text in");
            }
            t
        }
    };
    let mode = if a.append { Mode::Append } else { Mode::Replace };
    let row = match &a.row {
        None => None,
        Some(key) => {
            let mut set = vec![];
            for c in &a.cell {
                let Some((k, v)) = c.split_once('=') else { anyhow::bail!("--cell wants COLUMN=VALUE, got '{c}'") };
                set.push((k.trim().to_string(), v.trim().to_string()));
            }
            let row = if set.is_empty() { Some(text.trim().to_string()) } else { None };
            Some(RowChange { key: key.clone(), set, row, add: a.append })
        }
    };
    let e = Edit {
        section: a.section.clone(),
        new_section: a.new_section.clone(),
        after: a.after.clone(),
        mode,
        text: text.clone(),
        row,
    };
    let out = edit::plan(&s.corpus, idx, &e)?;
    let id = s.corpus.docs[idx].id.to_string();
    let verb = if a.append { "append to" } else { "replace" };
    let shown: Vec<&str> = out.after.lines().take(SHOWN).collect();
    let more = out.after.lines().count().saturating_sub(SHOWN);
    let mut o = Output::new("edit")
        .unpaged()
        .field("doc", id.clone())
        .field("target", out.target.clone())
        .field("mode", if a.append { "append" } else { "replace" })
        .field("lines_before", out.before.lines().count())
        .field("lines_after", out.after.lines().count())
        .field("after", out.after.clone())
        .head(format!("{verb} {} of {id}", out.target));
    for l in &shown {
        o = o.head(format!("  {l}"));
    }
    if more > 0 {
        o = o.head(format!("  … {more} more line(s)"));
    }
    let mut rerun = format!("spec edit {}", sh(&a.doc));
    if let Some(sec) = &a.section {
        rerun.push_str(&format!(" --section {}", sh(sec)));
    }
    if let Some(n) = &a.new_section {
        rerun.push_str(&format!(" --new-section {}", sh(n)));
    }
    if let Some(n) = &a.after {
        rerun.push_str(&format!(" --after {}", sh(n)));
    }
    if let Some(r) = &a.row {
        rerun.push_str(&format!(" --row {}", sh(r)));
    }
    for c in &a.cell {
        rerun.push_str(&format!(" --cell {}", sh(c)));
    }
    if a.append {
        rerun.push_str(" --append");
    }
    match (&a.text, &a.file) {
        _ if cells_only => {}
        (Some(t), _) => rerun.push_str(&format!(" --text={}", sh(t))),
        (None, Some(p)) if p.as_os_str() == "-" => rerun.push_str(" --file <saved-input>"),
        (None, None) => rerun.push_str(" --file <saved-input>"),
        (None, Some(p)) => rerun.push_str(&format!(" --file {}", sh(&p.display().to_string()))),
    }
    rerun.push_str(" --write");
    let o = preview::finish(o, &out.plan, &s.corpus.cfg.root, a.write, &rerun)?;
    Ok(if a.write { o.next(format!("spec check  # then: spec show {id}")) } else { o })
}

/// Quotes for a POSIX shell when needed, so the rerun hint can be pasted as is.
fn sh(s: &str) -> String {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "-_./:@=+,".contains(c)) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn quotes_for_the_shell() {
        assert_eq!(super::sh("adr:shop/1"), "adr:shop/1");
        assert_eq!(super::sh("it's ok"), "'it'\\''s ok'");
    }
}
