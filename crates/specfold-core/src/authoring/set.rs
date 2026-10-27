//! Changing frontmatter through assignments (`key=value`, `key+=value`, `key-=value`), with the
//! lifecycle enforced and `updated` kept current.

use super::plan::Plan;
use crate::corpus::Corpus;
use crate::markdown::frontmatter::edit;
use crate::model::{lifecycle, stamp};
use anyhow::{Result, anyhow, bail};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    Set,
    Add,
    Remove,
}

#[derive(Debug, Clone)]
pub struct Assign {
    /// `status`, `phase`, `tags`, `rel.implements` …
    pub key: String,
    pub op: Op,
    pub value: String,
}

impl Assign {
    pub fn parse(s: &str) -> Result<Self> {
        for (token, op) in [("+=", Op::Add), ("-=", Op::Remove), ("=", Op::Set)] {
            if let Some((k, v)) = s.split_once(token) {
                if k.is_empty() {
                    break;
                }
                return Ok(Assign { key: k.trim().to_string(), op, value: v.trim().to_string() });
            }
        }
        bail!("'{s}' is not key=value, key+=value or key-=value")
    }
}

const PROTECTED: &[&str] = &["id", "created", "files"];
const LIST_KEYS: &[&str] = &["tags", "owners", "aliases", "verify", "check_skip", "screens"];

fn target<'a>(map: &'a mut Map<String, Value>, key: &str) -> (&'a mut Map<String, Value>, String) {
    match key.split_once('.') {
        Some((parent, child)) => {
            let entry = map.entry(parent.to_string()).or_insert_with(|| Value::Object(Map::new()));
            if !entry.is_object() {
                *entry = Value::Object(Map::new());
            }
            (entry.as_object_mut().unwrap(), child.to_string())
        }
        None => (map, key.to_string()),
    }
}

fn apply_one(map: &mut Map<String, Value>, a: &Assign) {
    let is_list = a.key.starts_with("rel.") || LIST_KEYS.contains(&a.key.as_str());
    let value = if a.key == "updated" && a.value == "today" { stamp::today() } else { a.value.clone() };
    let (m, k) = target(map, &a.key);
    match (&a.op, is_list) {
        (Op::Set, true) => {
            m.insert(
                k,
                Value::Array(
                    value
                        .split(',')
                        .map(|s| Value::String(s.trim().into()))
                        .filter(|v| v.as_str() != Some(""))
                        .collect(),
                ),
            );
        }
        (Op::Set, false) => {
            m.insert(k, Value::String(value));
        }
        (Op::Add, _) => {
            let list = m.entry(k).or_insert_with(|| Value::Array(vec![]));
            if let Value::Array(items) = list
                && !items.iter().any(|x| x.as_str() == Some(value.as_str()))
            {
                items.push(Value::String(value));
            }
        }
        (Op::Remove, _) => {
            if let Some(Value::Array(items)) = m.get_mut(&k) {
                items.retain(|x| x.as_str() != Some(value.as_str()));
            }
        }
    }
}

pub fn plan(c: &Corpus, idx: usize, assigns: &[Assign]) -> Result<Plan> {
    let d = &c.docs[idx];
    let ty = c.cfg.registry.get(&d.id.ty).ok_or_else(|| anyhow!("unknown type {}", d.id.ty))?;
    for a in assigns {
        if PROTECTED.contains(&a.key.as_str()) {
            bail!(
                "{} cannot be set by hand: id comes from the path (spec mv), created is fixed, files is managed by spec attach",
                a.key
            );
        }
        if a.key == "status" {
            if !ty.statuses.contains(&a.value) {
                bail!("status '{}' is not allowed for {} (allowed: {})", a.value, ty.name, ty.statuses.join(", "));
            }
            let from = d.status().unwrap_or_else(|| ty.initial.clone());
            if !lifecycle::transition_allowed(&from, &a.value) {
                bail!(
                    "status cannot move from '{from}' to '{}'; supersede it with spec new {} --from {}",
                    a.value,
                    ty.name,
                    d.id
                );
            }
        }
    }
    let text = std::fs::read_to_string(c.cfg.root.join(&d.path))?;
    let new = edit::rewrite(&text, |m| {
        for a in assigns {
            apply_one(m, a);
        }
        if !assigns.iter().any(|a| a.key == "updated") {
            m.insert("updated".into(), Value::String(stamp::today()));
        }
        let ordered = edit::ordered(std::mem::take(m));
        *m = ordered;
    })?;
    let mut p = Plan::default();
    p.write(&d.path, new, false);
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_applies() {
        let mut m = Map::new();
        apply_one(&mut m, &Assign::parse("rel.implements+=sot:a/b").unwrap());
        apply_one(&mut m, &Assign::parse("rel.implements+=sot:a/b").unwrap());
        apply_one(&mut m, &Assign::parse("tags=x, y").unwrap());
        apply_one(&mut m, &Assign::parse("phase=P3").unwrap());
        assert_eq!(m["rel"]["implements"], serde_json::json!(["sot:a/b"]));
        assert_eq!(m["tags"], serde_json::json!(["x", "y"]));
        apply_one(&mut m, &Assign::parse("tags-=x").unwrap());
        assert_eq!(m["tags"], serde_json::json!(["y"]));
        assert!(Assign::parse("=x").is_err());
    }
}
