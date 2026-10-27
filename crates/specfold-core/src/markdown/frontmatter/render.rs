//! Writes frontmatter in a stable, readable YAML form. Only the shapes specfold writes are
//! supported: scalars, lists of scalars, lists of flat maps, and nested maps.

use serde_json::{Map, Value};

pub fn render(map: &Map<String, Value>) -> String {
    let mut out = String::from("---\n");
    for (k, v) in map {
        entry(&mut out, k, v, 0);
    }
    out.push_str("---\n");
    out
}

fn entry(out: &mut String, key: &str, v: &Value, indent: usize) {
    let pad = " ".repeat(indent);
    match v {
        Value::Object(m) if !m.is_empty() => {
            out.push_str(&format!("{pad}{key}:\n"));
            for (k2, v2) in m {
                entry(out, k2, v2, indent + 2);
            }
        }
        Value::Array(a) if a.iter().any(Value::is_object) => {
            out.push_str(&format!("{pad}{key}:\n"));
            for item in a {
                out.push_str(&format!("{pad}  - {}\n", inline(item)));
            }
        }
        _ => out.push_str(&format!("{pad}{key}: {}\n", inline(v))),
    }
}

fn inline(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => scalar(s),
        Value::Array(a) => format!("[{}]", a.iter().map(inline).collect::<Vec<_>>().join(", ")),
        Value::Object(m) => {
            let parts: Vec<String> = m.iter().map(|(k, v)| format!("{k}: {}", inline(v))).collect();
            format!("{{{}}}", parts.join(", "))
        }
    }
}

const RISKY_START: &[char] =
    &[' ', '-', '?', ':', ',', '[', ']', '{', '}', '#', '&', '*', '!', '|', '>', '\'', '"', '%', '@', '`'];

fn scalar(s: &str) -> String {
    let plain = !s.is_empty()
        && !s.starts_with(RISKY_START)
        && !s.contains(": ")
        && !s.contains(" #")
        && !s.contains([',', '[', ']', '{', '}', '\n', '\t'])
        && !s.ends_with([' ', ':'])
        && !matches!(s.to_ascii_lowercase().as_str(), "true" | "false" | "null" | "yes" | "no" | "on" | "off" | "~")
        && s.parse::<f64>().is_err();
    if plain { s.to_string() } else { serde_json::to_string(s).unwrap_or_default() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::frontmatter::{Parsed, parse, split};

    #[test]
    fn round_trip() {
        let m: Map<String, Value> = serde_json::from_value(serde_json::json!({
            "id": "plan:shop/20260101000000",
            "title": "Ship: the cart, fast",
            "created": "2026-01-01",
            "tags": ["a", "b"],
            "rel": {"governs": ["services/shop/**"]},
            "files": [{"path": "assets/x.png", "bytes": 3}]
        }))
        .unwrap();
        let text = format!("{}body\n", render(&m));
        let Parsed::Map(back) = parse(split(&text).raw) else { panic!("{text}") };
        assert_eq!(back, m);
    }
}
