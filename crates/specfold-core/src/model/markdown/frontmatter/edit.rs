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

/// E3: like [`rewrite`], but leaves untouched bytes alone. A change that leaves the map as it
/// was returns `text` as is; otherwise top-level scalar lines that changed are replaced in
/// place and `renames` are swapped as whole YAML tokens, so quoting and order elsewhere stay.
/// When that text does not parse back to the changed map, the whole block is rendered.
pub fn rewrite_keeping(
    text: &str,
    renames: &[(&str, &str)],
    change: impl FnOnce(&mut Map<String, Value>),
) -> Result<String> {
    let s = split(text);
    let map = match parse(s.raw) {
        Parsed::Map(m) => m,
        Parsed::None => Map::new(),
        Parsed::Invalid(e) => bail!("frontmatter does not parse: {e}"),
    };
    let mut new = map.clone();
    change(&mut new);
    if new == map {
        return Ok(text.to_string());
    }
    if let Some(raw) = s.raw {
        // Token renames first, as they keep quoting inside lists; then whole key blocks.
        for patched in [patch_raw(raw, &map, &new, renames), patch_blocks(raw, &map, &new)] {
            if matches!(parse(Some(&patched)), Parsed::Map(m) if m == new) {
                let head_len = text.len() - s.body.len();
                let fence_end = &text[..head_len];
                let start = fence_end.find(raw).unwrap_or(0);
                return Ok(format!("{}{}{}{}", &fence_end[..start], patched, &fence_end[start + raw.len()..], s.body));
            }
        }
    }
    Ok(format!("{}{}", render(&new), s.body))
}

/// The top-level key a frontmatter line starts, if it starts one.
fn top_key(line: &str) -> Option<&str> {
    line.split_once(':').map(|(k, _)| k).filter(|k| !k.is_empty() && !k.starts_with([' ', '\t', '-', '#']))
}

/// Each top-level key whose value changed is rendered again in its place, with the indented or
/// list lines under it; removed keys drop out and new keys go at the end.
fn patch_blocks(raw: &str, old: &Map<String, Value>, new: &Map<String, Value>) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut skipping = false;
    for line in raw.split_inclusive('\n') {
        let key = top_key(line);
        if skipping && key.is_none() && line.starts_with([' ', '\t', '-']) {
            continue;
        }
        skipping = false;
        match key.filter(|k| old.get(*k) != new.get(*k)) {
            Some(k) => {
                if let Some(v) = new.get(k) {
                    out.push_str(&one_key(k, v));
                }
                skipping = true;
            }
            None => out.push_str(line),
        }
    }
    for (k, v) in new.iter().filter(|(k, _)| !old.contains_key(*k)) {
        out.push_str(&one_key(k, v));
    }
    out
}

fn patch_raw(raw: &str, old: &Map<String, Value>, new: &Map<String, Value>, renames: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(raw.len());
    for line in raw.split_inclusive('\n') {
        let key = line.split_once(':').map(|(k, _)| k).filter(|k| !k.starts_with([' ', '\t', '-', '#']));
        let changed = key.and_then(|k| {
            new.get(k).filter(|v| old.get(k) != Some(*v) && !v.is_array() && !v.is_object()).map(|v| (k, v))
        });
        match changed {
            Some((k, v)) => {
                out.push_str(&one_key(k, v));
            }
            None => out.push_str(&rename_tokens(line, renames)),
        }
    }
    for (k, v) in new.iter().filter(|(k, _)| !old.contains_key(*k)) {
        out.push_str(&one_key(k, v));
    }
    out
}

fn one_key(k: &str, v: &Value) -> String {
    let mut one = Map::new();
    one.insert(k.to_string(), v.clone());
    let r = render(&one);
    r.strip_prefix("---\n").and_then(|r| r.strip_suffix("---\n")).unwrap_or(&r).to_string()
}

fn rename_tokens(line: &str, renames: &[(&str, &str)]) -> String {
    let edge = |c: Option<char>| c.is_none_or(|c| matches!(c, ' ' | '[' | ']' | ',' | '\'' | '"' | '#' | '\n' | '\r'));
    let mut s = line.to_string();
    for (from, to) in renames {
        let mut at = 0;
        while let Some(i) = s[at..].find(from).map(|i| i + at) {
            let end = i + from.len();
            if edge(s[..i].chars().next_back()) && edge(s[end..].chars().next()) {
                s.replace_range(i..end, to);
                at = i + to.len();
            } else {
                at = end;
            }
        }
    }
    s
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
    fn e3_keeping_leaves_quotes_and_untouched_files() {
        let text = "---\nid: sot:a/b\ntitle: 'Blog rules'\nrel:\n  depends_on: [sot:x/y, 'sot:x/yz']\n---\nbody\n";
        let same = rewrite_keeping(text, &[("sot:q/q", "sot:r/r")], |_| {}).unwrap();
        assert_eq!(same, text);
        let out = rewrite_keeping(text, &[("sot:x/y", "sot:z/y")], |m| {
            crate::authoring::refs::rewrite_rel(m, "sot:x/y", "sot:z/y");
        })
        .unwrap();
        assert_eq!(out, text.replace("[sot:x/y,", "[sot:z/y,"));
        let out = rewrite_keeping(text, &[], |m| {
            m.insert("updated".into(), Value::String("2026".into()));
            m.insert("id".into(), Value::String("sot:c/d".into()));
        })
        .unwrap();
        assert!(out.contains("title: 'Blog rules'\n") && out.contains("id: sot:c/d\n") && out.contains("updated:"));
    }

    #[test]
    fn keeping_changes_lists_and_maps_in_place_and_adds_keys_last() {
        let text =
            "---\nid: a\nverify_note: 'kept as is'\nrel:\n  derives:\n    - x\nupdated: 2026\ntags: [a]\n---\nbody\n";
        let out = rewrite_keeping(text, &[], |m| {
            m.insert("tags".into(), serde_json::json!(["a", "b"]));
            m["rel"]["derives"] = serde_json::json!(["x", "y"]);
            m.insert("decided_because".into(), Value::String("z".into()));
        })
        .unwrap();
        assert_eq!(
            out,
            "---\nid: a\nverify_note: 'kept as is'\nrel:\n  derives: [x, y]\nupdated: 2026\ntags: [a, b]\ndecided_because: z\n---\nbody\n"
        );
        let out = rewrite_keeping(text, &[], |m| {
            m.remove("rel");
        })
        .unwrap();
        assert_eq!(out, "---\nid: a\nverify_note: 'kept as is'\nupdated: 2026\ntags: [a]\n---\nbody\n");
    }

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
