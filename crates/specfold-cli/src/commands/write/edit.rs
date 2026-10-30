//! `spec edit`.

use super::preview;
use crate::cli::args::{global::Global, write::EditArgs};
use crate::output::Output;
use crate::session;
use anyhow::{Context, Result};
use specfold_core::authoring::edit::{self, Edit, Mode};
use specfold_core::markdown::tables::RowChange;
use specfold_core::shell::quote;
use std::io::Read;

const SHOWN: usize = 40;

pub fn run(g: &Global, a: &EditArgs) -> Result<Output> {
    let s = session::open(g)?;
    let idx = session::resolve(&s, &a.doc)?;
    if a.text.as_deref() == Some("-") {
        anyhow::bail!(
            "--text - would write a lone '-'; standard input is --file -; next: {}",
            rerun_with(a, " --file - --write")
        );
    }
    let cells_only = a.row.is_some() && !a.cell.is_empty() && a.text.is_none() && a.file.is_none();
    let text = match (&a.text, &a.file) {
        _ if cells_only => String::new(),
        (Some(t), _) => t.clone(),
        (None, Some(p)) if p.as_os_str() == "-" => {
            let mut t = String::new();
            std::io::stdin().read_to_string(&mut t).context("reading standard input")?;
            t
        }
        (None, Some(p)) => {
            std::fs::read_to_string(session::arg_path(g, p)).with_context(|| format!("reading {}", p.display()))?
        }
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
    let cells: String = a.cell.join("\n");
    let path = &s.corpus.docs[idx].path;
    crate::commands::outbound::refuse_redacted(&s.corpus.cfg.root, path, &format!("{text}\n{cells}"))?;
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
        text,
        row,
    };
    let out = match edit::plan(&s.corpus, idx, &e) {
        Ok(o) => o,
        Err(err) => {
            let msg = format!("{err:#}");
            // A missing section: the next step is the same edit adding it.
            if let (Some(sec), Some((head, _))) = (&a.section, msg.split_once("; to add it: --new-section")) {
                let mut cmd = format!("spec edit {} --new-section {}", quote(&a.doc), quote(sec));
                cmd.push_str(&text_flag(a, false));
                cmd.push_str(" --write");
                anyhow::bail!("{head}; next: {cmd}");
            }
            return Err(err);
        }
    };
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
    let mut rerun = format!("spec edit {}", quote(&a.doc));
    if let Some(sec) = &a.section {
        rerun.push_str(&format!(" --section {}", quote(sec)));
    }
    if let Some(n) = &a.new_section {
        rerun.push_str(&format!(" --new-section {}", quote(n)));
    }
    if let Some(n) = &a.after {
        rerun.push_str(&format!(" --after {}", quote(n)));
    }
    if let Some(r) = &a.row {
        rerun.push_str(&format!(" --row {}", quote(r)));
    }
    for c in &a.cell {
        rerun.push_str(&format!(" --cell {}", quote(c)));
    }
    if a.append {
        rerun.push_str(" --append");
    }
    rerun.push_str(&text_flag(a, cells_only));
    rerun.push_str(" --write");
    let o = preview::finish(o, &out.plan, &s.corpus.cfg.root, a.write, &rerun)?;
    Ok(if a.write { o.next(format!("spec check  # then: spec show {}", specfold_core::shell::word(&id))) } else { o })
}

/// Where the text came from, as flags for a command to paste.
fn text_flag(a: &EditArgs, cells_only: bool) -> String {
    match (&a.text, &a.file) {
        _ if cells_only => String::new(),
        (Some(t), _) => format!(" --text={}", quote(t)),
        (None, Some(p)) if p.as_os_str() == "-" => " --file <saved-input>".into(),
        (None, None) => " --file <saved-input>".into(),
        (None, Some(p)) => format!(" --file {}", quote(&p.display().to_string())),
    }
}

/// The command as given, without its text, and `tail` appended.
fn rerun_with(a: &EditArgs, tail: &str) -> String {
    let mut c = format!("spec edit {}", quote(&a.doc));
    for (flag, v) in
        [("--section", &a.section), ("--new-section", &a.new_section), ("--after", &a.after), ("--row", &a.row)]
    {
        if let Some(v) = v {
            c.push_str(&format!(" {flag} {}", quote(v)));
        }
    }
    if a.append {
        c.push_str(" --append");
    }
    c.push_str(tail);
    c
}

#[cfg(test)]
mod tests {
    #[test]
    fn quotes_for_the_shell() {
        assert_eq!(super::quote("adr:shop/1"), "adr:shop/1");
        assert_eq!(super::quote("it's ok"), "'it'\\''s ok'");
    }
}
