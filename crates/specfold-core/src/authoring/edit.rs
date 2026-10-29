//! Changing a document's body: replace or append to one section, or to the whole body. Without
//! this, agents edited files by hand and the `updated` date and lifecycle went unchecked.

use super::plan::Plan;
use crate::corpus::Corpus;
use crate::markdown::{frontmatter, frontmatter::edit, sections};
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
    pub mode: Mode,
    pub text: String,
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
    let text = std::fs::read_to_string(c.cfg.root.join(&d.path))?;
    let split = frontmatter::split(&text);
    let head = &text[..text.len() - split.body.len()];
    let lines: Vec<&str> = split.body.lines().collect();
    let new_text = e.text.trim_end_matches('\n');
    let (target, before, body) = match &e.section {
        Some(name) => {
            let Some((start, end)) = sections::span(split.body, name) else {
                let have: Vec<String> = sections::outline(split.body).into_iter().map(|(_, t)| t).collect();
                bail!("no section '{name}' (sections: {})", have.join(" | "));
            };
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
        m.insert("updated".into(), Value::String(stamp::today()));
    })?;
    let mut plan = Plan::default();
    if rewritten != text {
        plan.write(&d.path, rewritten, false);
    }
    Ok(Outcome { plan, target, before, after })
}
