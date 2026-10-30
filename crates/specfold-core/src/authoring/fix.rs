//! Applying the fixes that need no judgement: exact replacements reported by rules, missing
//! frontmatter keys, and a chosen target for an ambiguous reference.

use super::plan::Plan;
use crate::check::finding::{Finding, Fix};
use crate::corpus::Corpus;
use crate::markdown::frontmatter::edit;
use crate::model::stamp;
use crate::workspace::vcs;
use anyhow::Result;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct Pick {
    /// The id to use instead of an ambiguous reference.
    pub id: String,
    pub path: String,
    pub line: usize,
}

fn replace_on_line(text: &str, line: usize, from: &str, to: &str) -> Option<String> {
    let mut lines: Vec<String> = text.split_inclusive('\n').map(str::to_string).collect();
    let l = lines.get_mut(line.checked_sub(1)?)?;
    if !l.contains(from) {
        return None;
    }
    *l = l.replacen(from, to, 1);
    Some(lines.concat())
}

/// Fills id, title, status, created and updated where they are missing.
fn fill_frontmatter(c: &Corpus, idx: usize, text: &str) -> Result<String> {
    let d = &c.docs[idx];
    let initial = c.cfg.registry.get(&d.id.ty).map(|t| t.initial.clone()).unwrap_or_else(|| "draft".into());
    let created = vcs::first_commit_stamp(&c.cfg.root, &d.path)
        .and_then(|s| stamp::normalize_date(&s))
        .unwrap_or_else(stamp::today);
    let title = d.title.clone().unwrap_or_else(|| d.slug.replace('-', " "));
    edit::rewrite(text, |m| {
        m.entry("id").or_insert_with(|| Value::String(d.id.to_string()));
        m.entry("title").or_insert_with(|| Value::String(title));
        m.entry("status").or_insert_with(|| Value::String(initial));
        m.entry("created").or_insert_with(|| Value::String(created));
        m.entry("updated").or_insert_with(|| Value::String(stamp::now_value()));
        let ordered = edit::ordered(std::mem::take(m));
        *m = ordered;
    })
}

/// Keeps `alias` on the document `keep` and removes it from every other document that carries
/// it, in frontmatter `aliases` and in the alias table. Returns the number of files changed.
fn drop_alias(c: &Corpus, texts: &mut BTreeMap<String, String>, alias: &str, keep: &str) -> Result<usize> {
    let mut changed = 0;
    for d in &c.docs {
        if d.id.to_string() == keep || !d.list_field("aliases").iter().any(|a| a == alias) {
            continue;
        }
        let Some(t) = texts.get(&d.path).cloned().or_else(|| std::fs::read_to_string(c.cfg.root.join(&d.path)).ok())
        else {
            continue;
        };
        let n = edit::rewrite(&t, |m| {
            if let Some(Value::Array(list)) = m.get_mut("aliases") {
                list.retain(|v| v.as_str() != Some(alias));
                if list.is_empty() {
                    m.remove("aliases");
                }
            }
        })?;
        if n != t {
            texts.insert(d.path.clone(), n);
            changed += 1;
        }
    }
    if let Some(file) = &c.aliases.file {
        let t = texts.get(file).cloned().or_else(|| std::fs::read_to_string(c.cfg.root.join(file)).ok());
        if let Some(t) = t {
            let n: String = t
                .split_inclusive('\n')
                .filter(|l| match l.trim_end().split_once('\t') {
                    Some((old, new)) => old != alias || new.trim() == keep,
                    None => true,
                })
                .collect();
            if n != t {
                texts.insert(file.clone(), n);
                changed += 1;
            }
        }
    }
    Ok(changed)
}

pub fn plan(c: &Corpus, findings: &[Finding], pick: Option<&Pick>) -> Result<(Plan, usize)> {
    let mut texts: BTreeMap<String, String> = BTreeMap::new();
    let mut applied = 0;
    let read = |texts: &mut BTreeMap<String, String>, p: &str| -> Option<String> {
        if let Some(t) = texts.get(p) {
            return Some(t.clone());
        }
        std::fs::read_to_string(c.cfg.root.join(p)).ok()
    };
    for f in findings {
        match (&f.fix, f.rule.as_str()) {
            (Some(Fix::Replace { path, line, from, to }), _) => {
                let Some(t) = read(&mut texts, path) else { continue };
                if let Some(n) = replace_on_line(&t, *line, from, to) {
                    texts.insert(path.clone(), n);
                    applied += 1;
                }
            }
            (_, "S1") if f.message.starts_with("missing") || f.message == "no frontmatter block" => {
                let Some(idx) = c.docs.iter().position(|d| d.path == f.path) else { continue };
                let Some(t) = read(&mut texts, &f.path) else { continue };
                texts.insert(f.path.clone(), fill_frontmatter(c, idx, &t)?);
                applied += 1;
            }
            (_, "L1") if !f.candidates.is_empty() => {
                let Some(p) = pick.filter(|p| p.path == f.path && p.line == f.line && f.candidates.contains(&p.id))
                else {
                    continue;
                };
                let Some(t) = read(&mut texts, &f.path) else { continue };
                let Some(target) = f.message.split("]]").next().and_then(|s| s.strip_prefix("[[")) else { continue };
                let target_base = target.split('#').next().unwrap_or(target);
                if let Some(n) = replace_on_line(&t, f.line, &format!("[[{target_base}"), &format!("[[{}", p.id)) {
                    texts.insert(f.path.clone(), n);
                    applied += 1;
                }
            }
            (_, "I2") if f.candidates.len() > 1 => {
                let Some(p) = pick.filter(|p| p.path == f.path && f.candidates.contains(&p.id)) else { continue };
                let Some(alias) = f.message.strip_prefix("alias '").and_then(|m| m.split_once("' names")).map(|x| x.0)
                else {
                    continue;
                };
                applied += drop_alias(c, &mut texts, alias, &p.id)?;
            }
            _ => {}
        }
    }
    let mut plan = Plan::default();
    for (path, content) in texts {
        plan.write(path, content, false);
    }
    Ok((plan, applied))
}

/// Writes `rel.translation_of` and `lang` into every translation the `[translations]` layout
/// pairs by name alone, so the pair survives a rename or a layout change.
pub fn translation_pairs(c: &Corpus) -> Result<(Plan, usize)> {
    let mut plan = Plan::default();
    let mut n = 0;
    for (i, d) in c.docs.iter().enumerate() {
        if crate::translation::source_ref(c, i).is_some() {
            continue;
        }
        let Some(inf) = crate::translation::inferred(c, i) else { continue };
        let Some(src) = inf.source else { continue };
        let Ok(t) = std::fs::read_to_string(c.cfg.root.join(&d.path)) else { continue };
        let source = c.docs[src].id.to_string();
        let new = edit::rewrite(&t, |m| {
            m.entry("lang").or_insert_with(|| Value::String(inf.lang.clone()));
            let rel = m.entry("rel").or_insert_with(|| Value::Object(Default::default()));
            if let Value::Object(r) = rel {
                r.insert(crate::translation::RELATION.into(), Value::Array(vec![Value::String(source.clone())]));
            }
            let ordered = edit::ordered(std::mem::take(m));
            *m = ordered;
        })?;
        if new != t {
            plan.write(d.path.clone(), new, false);
            n += 1;
        }
    }
    Ok((plan, n))
}
