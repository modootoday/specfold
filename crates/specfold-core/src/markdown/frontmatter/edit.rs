//! Rewrites a file's frontmatter while keeping its body byte for byte.

use super::{Parsed, parse, render, split};
use anyhow::{Result, bail};
use serde_json::{Map, Value};

/// Applies `change` to the frontmatter of `text`. A file without frontmatter gets one.
pub fn rewrite(text: &str, change: impl FnOnce(&mut Map<String, Value>)) -> Result<String> {
    let s = split(text);
    let mut map = match parse(s.raw) {
        Parsed::Map(m) => m,
        Parsed::None => Map::new(),
        Parsed::Invalid(e) => bail!("frontmatter does not parse: {e}"),
    };
    change(&mut map);
    Ok(format!("{}{}", render(&map), s.body))
}

/// Puts the common keys first in a fixed order so diffs stay small; other keys keep their order.
pub fn ordered(map: Map<String, Value>) -> Map<String, Value> {
    const ORDER: &[&str] = &["id", "title", "status", "created", "updated", "owners", "tags", "aliases"];
    let mut out = Map::new();
    let mut rest = map;
    for k in ORDER {
        if let Some(v) = rest.remove(*k) {
            out.insert((*k).to_string(), v);
        }
    }
    let tail: Vec<(String, Value)> = rest.into_iter().collect();
    let (late, early): (Vec<_>, Vec<_>) = tail.into_iter().partition(|(k, _)| k == "rel" || k == "files");
    out.extend(early);
    out.extend(late);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_body() {
        let text = "---\nid: a\nstatus: draft\n---\n# Title\n\nbody `with` --- dashes\n";
        let out = rewrite(text, |m| {
            m.insert("status".into(), Value::String("active".into()));
        })
        .unwrap();
        assert!(out.ends_with("# Title\n\nbody `with` --- dashes\n"));
        assert!(out.contains("status: active\n"));
    }
}
