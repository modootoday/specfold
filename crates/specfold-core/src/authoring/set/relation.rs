//! Relation keys written without their `rel.` prefix: `governs=x` means `rel.governs=x`.

use super::Assign;
use crate::corpus::graph::DOC_RELATIONS;
use crate::model::doctype::TypeDef;
use serde_json::{Map, Value};

/// Whether `key` names a relation, which documents keep under `rel:`.
fn is_relation(key: &str) -> bool {
    key == "governs" || DOC_RELATIONS.contains(&key)
}

/// `a` with a bare relation key moved under `rel.`, unless the type declares that key as a field.
pub fn qualified(ty: &TypeDef, a: &Assign) -> Assign {
    let mut a = a.clone();
    if is_relation(&a.key) && !ty.fields.contains(&a.key) {
        a.key = format!("rel.{}", a.key);
    }
    a
}

/// Moves a top-level relation key into `rel.<kind>` before `rel.<kind>` is written, so the
/// document keeps one list instead of two that disagree.
pub fn fold_top_level(map: &mut Map<String, Value>, key: &str) {
    let Some(kind) = key.strip_prefix("rel.").filter(|k| is_relation(k)) else { return };
    let Some(loose) = map.remove(kind) else { return };
    let rel = map.entry("rel").or_insert_with(|| Value::Object(Map::new()));
    if !rel.is_object() {
        *rel = Value::Object(Map::new());
    }
    let rel = rel.as_object_mut().expect("rel was just made an object");
    let mut items = match rel.remove(kind) {
        Some(Value::Array(v)) => v,
        Some(Value::String(s)) => vec![Value::String(s)],
        _ => vec![],
    };
    let extra = match loose {
        Value::Array(v) => v,
        Value::String(s) => s.split(',').map(|p| Value::String(p.trim().to_string())).collect(),
        _ => vec![],
    };
    for v in extra {
        if v.as_str().is_some_and(|s| !s.is_empty()) && !items.contains(&v) {
            items.push(v);
        }
    }
    rel.insert(kind.to_string(), Value::Array(items));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn fold_top_level_merges_a_loose_governs_into_rel() {
        let mut m = json!({"governs": "services/shop", "rel": {"governs": ["libs/auth"]}});
        let m = m.as_object_mut().unwrap();
        fold_top_level(m, "rel.governs");
        assert!(m.get("governs").is_none());
        assert_eq!(m["rel"]["governs"], json!(["libs/auth", "services/shop"]));
    }

    #[test]
    fn fold_top_level_leaves_other_keys_alone() {
        let mut m = json!({"tags": ["x"]});
        let m = m.as_object_mut().unwrap();
        fold_top_level(m, "rel.tags");
        fold_top_level(m, "tags");
        assert_eq!(m["tags"], json!(["x"]));
        assert!(m.get("rel").is_none());
    }
}
