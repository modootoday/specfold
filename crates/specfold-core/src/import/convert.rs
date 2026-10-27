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

/// A document named in an old relation, without the notes and extensions people wrote around it:
/// `ADR-7-use-files.md (platform clauses only)` becomes `ADR-7-use-files`.
fn document_target(raw: &str) -> String {
    let mut s = raw.trim();
    if let Some(i) = s.find(" (") {
        s = &s[..i];
    }
    s.trim_end_matches(".md").trim().to_string()
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
    let mut fallbacks = vec![];
    for (k, v) in old {
        let target = map.keys.get(k).map(String::as_str).unwrap_or(k.as_str());
        if let Some(t) = target.strip_suffix('?') {
            fallbacks.push((t, v));
            continue;
        }
        match target {
            "-" | "id" | "status" | "created" | "updated" | "title" => {}
            t if t.starts_with("rel.") => {
                let kind = &t[4..];
                let list = rel.entry(kind.to_string()).or_insert_with(|| Value::Array(vec![]));
                if let Value::Array(items) = list {
                    let clean = |s: String| if kind == "governs" { s } else { document_target(&s) };
                    items.extend(list_of(Some(v)).into_iter().map(clean).map(Value::String));
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
    for (t, v) in fallbacks {
        fm.entry(t.to_string()).or_insert_with(|| v.clone());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fallback_key_fills_only_what_nothing_else_set() {
        let map: ImportMap =
            toml::from_str("[keys]\ncheck_cmd = \"verify\"\ncheck = \"verify?\"\nverify = \"verify_note\"\n").unwrap();
        let rule: SourceRule = toml::from_str("glob = \"x\"\ntype = \"sot\"").unwrap();
        let allowed = ["active".to_string()];
        let convert = |yaml: &str| {
            let crate::markdown::frontmatter::Parsed::Map(old) = crate::markdown::frontmatter::parse(Some(yaml)) else {
                panic!("fixture does not parse")
            };
            frontmatter(&map, &rule, &old, &allowed, "t").fm
        };
        let both = convert("check: manual\ncheck_cmd: cargo test\nverify: prose\nstatus: active\n");
        assert_eq!((both["verify"].as_str(), both["verify_note"].as_str()), (Some("cargo test"), Some("prose")));
        let manual = convert("check: manual\nstatus: active\n");
        assert_eq!(manual["verify"], "manual");
    }

    #[test]
    fn relation_targets_lose_notes_and_extensions_but_governs_keeps_paths() {
        let map: ImportMap =
            toml::from_str("[keys]\nsupersedes = \"rel.supersedes\"\napplies_to = \"rel.governs\"\n").unwrap();
        let rule: SourceRule = toml::from_str("glob = \"x\"\ntype = \"adr\"").unwrap();
        let yaml = "status: active\nsupersedes: [\"ADR-7-use-files.md (platform clauses only)\"]\napplies_to: [\"docs/a.md\"]\n";
        let crate::markdown::frontmatter::Parsed::Map(old) = crate::markdown::frontmatter::parse(Some(yaml)) else {
            panic!("fixture does not parse")
        };
        let fm = frontmatter(&map, &rule, &old, &["active".to_string()], "t").fm;
        assert_eq!(fm["rel"]["supersedes"][0], "ADR-7-use-files");
        assert_eq!(fm["rel"]["governs"][0], "docs/a.md");
    }
}
