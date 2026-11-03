//! The plan over every source unit: document rows, dropped sources and redirect stubs, then the
//! links between the imported documents.

use super::links::{canonical_links, link_replacements, rewrite_links};
use super::names::stem_of;
use super::row::doc_row;
use super::types::{ImportPlan, Row};
use crate::corpus::Corpus;
use crate::import::map::ImportMap;
use crate::import::sources;
use anyhow::{Result, anyhow};
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};

pub fn build(c: &Corpus, map: &ImportMap) -> Result<ImportPlan> {
    let units = sources::expand(&c.cfg.root, map);
    let mut out = ImportPlan::default();
    let mut taken: BTreeSet<String> = c.docs.iter().map(|d| d.id.to_string()).collect();
    let mut by_source: BTreeMap<String, String> = BTreeMap::new();
    let mut redirects = vec![];
    for u in &units {
        let rule = &map.sources[u.rule];
        if let Some(re) = &rule.redirect {
            redirects.push((u.clone(), Regex::new(re).map_err(|e| anyhow!("bad redirect regex: {e}"))?));
            continue;
        }
        if rule.drop {
            out.plan.remove(u.source.clone());
            out.rows.push(Row { source: u.source.clone(), kind: "drop".into(), ..Row::default() });
            continue;
        }
        let row = doc_row(c, map, u, &mut taken, &mut out)?;
        for p in [&u.main, &u.source].into_iter().chain(&u.chapters).chain(&u.attachments) {
            by_source.insert(p.clone(), row.id.clone());
        }
        if map.stem_aliases {
            out.alias_rows.push((stem_of(&u.source).to_string(), row.id.clone()));
        }
        out.rows.push(row);
    }
    for (u, re) in redirects {
        let text = std::fs::read_to_string(c.cfg.root.join(&u.main)).unwrap_or_default();
        let target = re
            .captures(&text)
            .and_then(|cap| cap.name("target"))
            .map(|m| m.as_str().trim().trim_start_matches("./").to_string());
        let resolved = target.as_ref().and_then(|t| {
            by_source.get(t).cloned().or_else(|| c.docs.iter().find(|d| &d.path == t).map(|d| d.id.to_string()))
        });
        let mut unresolved = vec![];
        match &resolved {
            Some(id) => {
                out.alias_rows.push((u.source.clone(), id.clone()));
                if map.stem_aliases {
                    out.alias_rows.push((stem_of(&u.source).to_string(), id.clone()));
                }
            }
            None => unresolved.push(format!("stub target {} is not imported", target.unwrap_or_else(|| "?".into()))),
        }
        out.plan.remove(u.source.clone());
        out.rows.push(Row {
            source: u.source.clone(),
            kind: "redirect".into(),
            id: resolved.unwrap_or_default(),
            path: String::new(),
            owner_reason: String::new(),
            status_from: String::new(),
            status_to: String::new(),
            aliases: vec![],
            unresolved,
            ..Row::default()
        });
    }
    canonical_links(&mut out, &by_source);
    link_replacements(&mut out);
    rewrite_links(&mut out, &by_source);
    Ok(out)
}
