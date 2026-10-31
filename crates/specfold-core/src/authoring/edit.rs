//! Changing a document's body: replace or append to one section, or to the whole body. Without
//! this, agents edited files by hand and the `updated` date and lifecycle went unchecked.

use super::plan::Plan;
use crate::corpus::Corpus;
use crate::markdown::tables::{self, RowChange};
use crate::markdown::{frontmatter, frontmatter::edit, sections};
use crate::model::doc::Doc;
use crate::model::stamp;
use anyhow::{Result, anyhow, bail};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The section's content (its heading kept) or the whole body.
    Replace,
    /// After the section's last line, or at the end of the body.
    Append,
}

#[derive(Debug, Clone)]
pub struct Edit {
    pub section: Option<String>,
    /// Add a new section with this heading instead of changing one; placed after `after`
    /// (and its subsections) at the same level, or at the end of the body at level 2.
    pub new_section: Option<String>,
    pub after: Option<String>,
    pub mode: Mode,
    pub text: String,
    /// Change one table row (by its first cell) instead of the section text.
    pub row: Option<RowChange>,
}

/// What an edit changed, for previews.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub plan: Plan,
    /// The section heading or `body`.
    pub target: String,
    pub before: String,
    pub after: String,
}

const CLOSED: &[&str] = &["superseded", "archived", "rejected"];

pub fn plan(c: &Corpus, idx: usize, e: &Edit) -> Result<Outcome> {
    let d = &c.docs[idx];
    let ty = c.cfg.registry.get(&d.id.ty).ok_or_else(|| anyhow!("unknown type {}", d.id.ty))?;
    if ty.sealed {
        bail!("{} is a sealed {}; record a new one instead of editing it", d.id, ty.name);
    }
    let status = d.status().unwrap_or_default();
    if CLOSED.contains(&status.as_str()) {
        bail!("{} is {status}; its body is history. Supersede it: spec new {} --from {}", d.id, ty.name, d.id);
    }
    let text = crate::corpus::overlay::read(&c.cfg.root.join(&d.path))?;
    let split = frontmatter::split(&text);
    let head = &text[..text.len() - split.body.len()];
    let lines: Vec<&str> = split.body.lines().collect();
    let new_text = e.text.trim_end_matches('\n');
    refuse_open_fence(new_text)?;
    if let Some(ch) = &e.row {
        return plan_row(d, head, &lines, split.body, e.section.as_deref(), ch);
    }
    if let Some(title) = &e.new_section {
        return plan_new_section(d, head, &lines, split.body, e.after.as_deref(), title, new_text);
    }
    let (target, before, body) = match &e.section {
        Some(name) => {
            let Some((start, end)) = sections::span(split.body, name) else {
                let msg = sections::not_found(split.body, split.body_line, name);
                if sections::matches(split.body, name).len() > 1 {
                    bail!("{msg}");
                }
                bail!("{msg}; to add it: --new-section {name:?}");
            };
            refuse_headings(new_text, lines[start].trim_start().chars().take_while(|c| *c == '#').count())?;
            // Trailing blank lines separate sections; keep them outside the edited content.
            let mut last = end;
            while last > start + 1 && lines[last - 1].trim().is_empty() {
                last -= 1;
            }
            let before = lines[start + 1..last].join("\n").trim_start_matches('\n').to_string();
            let content = match e.mode {
                Mode::Replace => new_text.to_string(),
                Mode::Append if before.trim().is_empty() => new_text.to_string(),
                Mode::Append => format!("{before}\n\n{new_text}"),
            };
            let mut out: Vec<String> = lines[..=start].iter().map(|s| s.to_string()).collect();
            out.push(String::new());
            out.extend(content.lines().map(str::to_string));
            if end < lines.len() {
                out.push(String::new());
            }
            out.extend(lines[end..].iter().map(|s| s.to_string()));
            (lines[start].trim().to_string(), before, out.join("\n") + "\n")
        }
        None => {
            let before = split.body.trim_end().to_string();
            let body = match e.mode {
                Mode::Replace => format!("{new_text}\n"),
                Mode::Append => format!("{before}\n\n{new_text}\n"),
            };
            ("body".to_string(), before, format!("\n{}", body.trim_start_matches('\n')))
        }
    };
    let after = match &e.section {
        Some(name) => sections::extract(&body, name).unwrap_or_default(),
        None => body.trim().to_string(),
    };
    let rewritten = edit::rewrite(&format!("{head}{body}"), |m| {
        m.insert("updated".into(), Value::String(stamp::now_value()));
    })?;
    let mut plan = Plan::default();
    if rewritten != text {
        plan.write(&d.path, rewritten, false);
    }
    Ok(Outcome { plan, target, before, after })
}

/// Words a duplicate-key error: rows spread over tables outside a named section ask for
/// --section; rows in one table (or a section already named) point at editing the whole section.
fn duplicate_rows(e: anyhow::Error, d: &Doc, head: &str, lines: &[&str], section: Option<&str>) -> anyhow::Error {
    let Some(dup) = e.downcast_ref::<tables::DuplicateRows>() else { return e };
    let first = head.matches('\n').count() + 1;
    let at: Vec<String> = dup.rows.iter().map(|r| (r + first).to_string()).collect();
    let id = crate::shell::word(&d.id);
    match section {
        None if dup.tables > 1 => anyhow::anyhow!(
            "'{}' names {} rows in {} tables (lines {}); name the section with --section; next: spec show {id}",
            dup.key,
            dup.rows.len(),
            dup.tables,
            at.join(", ")
        ),
        _ => {
            let heading = || {
                let above = dup.rows.first().map_or(0, |r| *r);
                lines[..above].iter().rev().find(|l| l.starts_with('#')).map(|l| l.trim_start_matches('#').trim())
            };
            let s = crate::shell::opt("--section", section.or_else(heading).unwrap_or("<section>"));
            anyhow::anyhow!(
                "several rows have the key '{}' (lines {}); --row edits one row; next: spec edit {id} {s}",
                dup.key,
                at.join(", ")
            )
        }
    }
}

/// `--row`: change one table row, in a section or anywhere in the body.
fn plan_row(d: &Doc, head: &str, lines: &[&str], body: &str, section: Option<&str>, ch: &RowChange) -> Result<Outcome> {
    let (lo, hi, target) = match &section {
        Some(name) => {
            let Some((start, end)) = sections::span(body, name) else {
                bail!("{}", sections::not_found(body, head.matches('\n').count() + 1, name));
            };
            (start + 1, end, format!("row '{}' in {}", ch.key, lines[start].trim()))
        }
        None => (0, lines.len(), format!("row '{}'", ch.key)),
    };
    let (out, before, after) =
        tables::edit_row(lines, lo, hi, ch).map_err(|e| duplicate_rows(e, d, head, lines, section))?;
    let body = out.join("\n") + "\n";
    let rewritten = edit::rewrite(&format!("{head}{body}"), |m| {
        m.insert("updated".into(), Value::String(stamp::now_value()));
    })?;
    let mut plan = Plan::default();
    if before != after {
        plan.write(&d.path, rewritten, false);
    }
    Ok(Outcome { plan, target, before, after })
}

/// `--new-section`: add a section after another one, or at the end.
fn plan_new_section(
    d: &Doc,
    head: &str,
    lines: &[&str],
    body: &str,
    after: Option<&str>,
    title: &str,
    new_text: &str,
) -> Result<Outcome> {
    let title = title.trim();
    if !sections::matches(body, title).is_empty()
        && sections::matches(body, title)
            .iter()
            .all(|&i| lines[i].trim_start().trim_start_matches('#').trim().eq_ignore_ascii_case(title))
    {
        bail!("a section '{title}' already exists; edit it with --section");
    }
    let (at, level) = match &after {
        Some(name) => {
            let Some((start, end)) = sections::span(body, name) else {
                bail!("{}", sections::not_found(body, head.matches('\n').count() + 1, name));
            };
            (end, lines[start].trim_start().chars().take_while(|c| *c == '#').count())
        }
        None => (lines.len(), 2),
    };
    refuse_headings(new_text, level)?;
    let mut prefix: Vec<String> = lines[..at].iter().map(|s| s.to_string()).collect();
    while prefix.last().is_some_and(|l| l.trim().is_empty()) {
        prefix.pop();
    }
    let mut out = prefix;
    out.push(String::new());
    out.push(format!("{} {title}", "#".repeat(level)));
    out.push(String::new());
    out.extend(new_text.lines().map(str::to_string));
    if at < lines.len() {
        out.push(String::new());
        out.extend(lines[at..].iter().map(|s| s.to_string()));
    }
    let body = format!("\n{}\n", out.join("\n").trim_start_matches('\n'));
    let rewritten = edit::rewrite(&format!("{head}{body}"), |m| {
        m.insert("updated".into(), Value::String(stamp::now_value()));
    })?;
    let mut plan = Plan::default();
    plan.write(&d.path, rewritten, false);
    Ok(Outcome { plan, target: format!("new section '{title}'"), before: String::new(), after: new_text.to_string() })
}

/// C208: text that opens a code fence and never closes it would turn every later heading of the
/// document into code.
fn refuse_open_fence(text: &str) -> Result<()> {
    let mut fence = crate::markdown::Fence::default();
    let mut opened = 0;
    for (n, line) in text.lines().enumerate() {
        if fence.step(line) && fence.inside() {
            opened = n + 1;
        }
    }
    if fence.inside() {
        bail!("the text opens a code fence at line {opened} that it never closes; close it with the same fence line");
    }
    Ok(())
}

/// C125: section text may hold sub-headings, but a heading at the section's level or above would
/// start another section (and could duplicate one).
fn refuse_headings(text: &str, level: usize) -> Result<()> {
    let mut fence = crate::markdown::Fence::default();
    for line in text.lines() {
        let t = line.trim_start();
        if fence.step(t) || fence.inside() {
            continue;
        }
        let hashes = t.chars().take_while(|c| *c == '#').count();
        if hashes >= 1 && hashes <= level && t[hashes..].starts_with(' ') {
            bail!(
                "the text holds the heading '{}', which would start a new section; add it with --new-section, or use a deeper heading ({})",
                t.trim(),
                "#".repeat(level + 1)
            );
        }
    }
    Ok(())
}
