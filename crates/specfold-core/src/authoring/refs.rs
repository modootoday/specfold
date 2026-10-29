//! Rewriting references to a document when its id changes.

use regex::Regex;
use serde_json::{Map, Value};

/// Replaces `[[old]]` and `[[old#anchor]]` (and `[[old|label]]`) with the new id in Markdown text.
pub fn rewrite_body(text: &str, old: &str, new: &str) -> String {
    let re = Regex::new(&format!(r"\[\[{}(#[^\]|]*)?(\|[^\]]*)?\]\]", regex::escape(old))).unwrap();
    re.replace_all(text, |c: &regex::Captures| {
        format!("[[{new}{}{}]]", c.get(1).map_or("", |m| m.as_str()), c.get(2).map_or("", |m| m.as_str()))
    })
    .into_owned()
}

/// Whether a cited path names `path` (root-relative), ignoring leading `./` and `../`.
pub fn same_path(cited: &str, path: &str) -> bool {
    let mut t = cited.trim_end_matches('/');
    while let Some(rest) = t.strip_prefix("../").or_else(|| t.strip_prefix("./")) {
        t = rest;
    }
    t == path
}

/// Replaces the id inside `rel.*` lists, keeping anchors.
pub fn rewrite_rel(map: &mut Map<String, Value>, old: &str, new: &str) -> bool {
    let Some(Value::Object(rel)) = map.get_mut("rel") else { return false };
    let mut changed = false;
    for v in rel.values_mut() {
        let Value::Array(items) = v else { continue };
        for item in items.iter_mut() {
            let Some(s) = item.as_str() else { continue };
            let (target, anchor) = s.split_once('#').map(|(t, a)| (t, Some(a))).unwrap_or((s, None));
            if target == old {
                *item = Value::String(match anchor {
                    Some(a) => format!("{new}#{a}"),
                    None => new.to_string(),
                });
                changed = true;
            }
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites() {
        let t = "a [[adr:x/1]] b [[adr:x/1#K-1|see]] c [[adr:x/10]]";
        assert_eq!(rewrite_body(t, "adr:x/1", "adr:y/1"), "a [[adr:y/1]] b [[adr:y/1#K-1|see]] c [[adr:x/10]]");
        let mut m: Map<String, Value> =
            serde_json::from_str(r#"{"rel":{"implements":["adr:x/1#A","sot:z/q"]}}"#).unwrap();
        assert!(rewrite_rel(&mut m, "adr:x/1", "adr:y/1"));
        assert_eq!(m["rel"]["implements"][0], "adr:y/1#A");
    }
}
