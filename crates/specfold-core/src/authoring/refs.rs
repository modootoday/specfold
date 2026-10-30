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

/// `a/./b/../c` as `a/c`; `None` when it climbs above the root.
fn normalize(path: &str) -> Option<String> {
    let mut out: Vec<&str> = vec![];
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                out.pop()?;
            }
            p => out.push(p),
        }
    }
    Some(out.join("/"))
}

/// The path of `to` written relative to the folder holding `from`.
fn relative(from: &str, to: &str) -> String {
    let base: Vec<&str> =
        from.rsplit_once('/').map(|x| x.0).unwrap_or("").split('/').filter(|p| !p.is_empty()).collect();
    let target: Vec<&str> = to.split('/').collect();
    let common = base.iter().zip(&target).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<&str> = vec![".."; base.len() - common];
    parts.extend(&target[common..]);
    parts.join("/")
}

/// Rewrites `[text](rel/path#frag)` links in `file` that point at `old` so they point at `new`.
pub fn rewrite_relative_links(body: &str, file: &str, old: &str, new: &str) -> String {
    let dir = file.rsplit_once('/').map(|x| x.0).unwrap_or("");
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(i) = rest.find("](") {
        out.push_str(&rest[..i + 2]);
        rest = &rest[i + 2..];
        let Some(end) = rest.find(')') else { break };
        let link = &rest[..end];
        let (target, frag) = match link.split_once('#') {
            Some((t, f)) => (t, Some(f)),
            None => (link, None),
        };
        let local = !target.is_empty() && !target.contains("://") && !target.starts_with('/') && !target.contains(' ');
        let joined = if dir.is_empty() { target.to_string() } else { format!("{dir}/{target}") };
        if local && normalize(&joined).as_deref() == Some(old) {
            out.push_str(&relative(file, new));
            if let Some(f) = frag {
                out.push('#');
                out.push_str(f);
            }
        } else {
            out.push_str(link);
        }
        out.push(')');
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
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
