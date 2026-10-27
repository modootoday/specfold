//! Converting an old document's frontmatter to the specfold shape.

use super::map::{ImportMap, SourceRule};
use crate::model::doc::list_of;
use crate::model::stamp;
use serde_json::{Map, Value};

pub struct Converted {
    pub fm: Map<String, Value>,
    pub status_from: String,
    pub aliases: Vec<String>,
    pub unresolved: Vec<String>,
}

pub fn status(map: &ImportMap, rule: &SourceRule, old: &str, allowed: &[String]) -> Option<String> {
    let mapped = rule.status.get(old).or_else(|| map.status.get(old)).cloned().unwrap_or_else(|| old.to_string());
    allowed.contains(&mapped).then_some(mapped)
}

pub fn frontmatter(
    map: &ImportMap,
    rule: &SourceRule,
    old: &Map<String, Value>,
    allowed: &[String],
    fallback_title: &str,
) -> Converted {
    let mut fm = Map::new();
    let mut rel = Map::new();
    let mut unresolved = vec![];
    let mut aliases: Vec<String> = rule.alias_fields.iter().flat_map(|f| list_of(old.get(f))).collect();
    for (k, v) in old {
        let target = map.keys.get(k).map(String::as_str).unwrap_or(k.as_str());
        match target {
            "-" | "id" | "status" | "created" | "updated" | "title" => {}
            t if t.starts_with("rel.") => {
                let kind = &t[4..];
                let list = rel.entry(kind.to_string()).or_insert_with(|| Value::Array(vec![]));
                if let Value::Array(items) = list {
                    items.extend(list_of(Some(v)).into_iter().map(Value::String));
                }
            }
            "aliases" => aliases.extend(list_of(Some(v))),
            "rel" => {
                if let Value::Object(m) = v {
                    rel.extend(m.clone());
                }
            }
            t => {
                fm.insert(t.to_string(), v.clone());
            }
        }
    }
    let status_from = old.get("status").and_then(Value::as_str).unwrap_or("").trim().to_string();
    let status_new = if status_from.is_empty() {
        rule.default_status.clone().filter(|s| allowed.contains(s))
    } else {
        status(map, rule, &status_from, allowed)
    };
    let status_new = status_new.unwrap_or_else(|| {
        unresolved.push(format!("status '{status_from}' has no mapping"));
        allowed.first().cloned().unwrap_or_else(|| "draft".into())
    });
    let date = |k: &str| {
        old.get(k)
            .and_then(|v| v.as_str().map(str::to_string).or_else(|| v.as_i64().map(|n| n.to_string())))
            .and_then(|s| stamp::normalize_date(&s))
    };
    let created = date("created").or_else(|| date("date"));
    let updated = date("updated").or_else(|| created.clone());
    let title =
        old.get("title").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| fallback_title.to_string());
    let mut out = Map::new();
    out.insert("title".into(), Value::String(title));
    out.insert("status".into(), Value::String(status_new));
    if let Some(c) = created {
        out.insert("created".into(), Value::String(c));
    }
    if let Some(u) = updated {
        out.insert("updated".into(), Value::String(u));
    }
    out.extend(fm);
    if !rel.is_empty() {
        out.insert("rel".into(), Value::Object(rel));
    }
    aliases.sort();
    aliases.dedup();
    Converted { fm: out, status_from, aliases, unresolved }
}
