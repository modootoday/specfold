//! Links between imported documents: replacements, canonical ids, and rewritten relative paths.

use super::*;

/// "superseded by X": the newer record, when this import names it, gets rel.supersedes to the
/// older one; otherwise the row needs a decision, since check would report T1 after the import.
pub(super) fn link_replacements(out: &mut ImportPlan) {
    let names: BTreeMap<String, String> = out.alias_rows.iter().map(|(o, n)| (o.clone(), n.clone())).collect();
    // A bare record number (`ADR-0002` → `0002`) also names the file that starts with it.
    let by_name = |c: &str| {
        let numbered = format!("{c}-");
        names.get(c).cloned().or_else(|| {
            names
                .iter()
                .find(|(o, _)| {
                    stem_of(o) == c || (c.chars().all(|ch| ch.is_ascii_digit()) && stem_of(o).starts_with(&numbered))
                })
                .map(|x| x.1.clone())
        })
    };
    let paths: BTreeMap<String, String> =
        out.rows.iter().filter(|r| r.kind == "doc").map(|r| (r.id.clone(), r.path.clone())).collect();
    for i in 0..out.rows.len() {
        let Some(by) = out.rows[i].replaced_by.clone() else { continue };
        let old = out.rows[i].id.clone();
        let newer = by_candidates(&by).iter().find_map(|c| by_name(c)).filter(|n| *n != old && paths.contains_key(n));
        match newer {
            Some(n) => {
                edit_frontmatter(&mut out.plan, &paths[&n], |fm| {
                    let rel = fm.entry("rel").or_insert_with(|| Value::Object(Map::new()));
                    if let Value::Object(rel) = rel
                        && let Value::Array(items) = rel.entry("supersedes").or_insert_with(|| Value::Array(vec![]))
                        && !items.iter().any(|v| v.as_str() == Some(&old))
                    {
                        items.push(Value::String(old.clone()));
                    }
                });
                out.rows[i].notes.push(format!("replaced by {n}: rel.supersedes added there"));
            }
            None => out.rows[i].unresolved.push(format!(
                "replaced by {by}, which this import does not include: add rel.supersedes: [{old}] to the newer record"
            )),
        }
    }
}

/// `rel.*` values that name an imported file by its old name (`0002-use-postgres`,
/// `0002-use-postgres.md`; document relations only, never paths) are written as the id it gets, so the
/// link does not depend on the alias; the alias still resolves the old name elsewhere.
pub(super) fn canonical_links(out: &mut ImportPlan, by_source: &BTreeMap<String, String>) {
    let mut names: BTreeMap<String, String> = BTreeMap::new();
    for (src, id) in
        by_source.iter().map(|(a, b)| (a.as_str(), b)).chain(out.alias_rows.iter().map(|(a, b)| (a.as_str(), b)))
    {
        names.entry(stem_of(src).to_string()).or_insert_with(|| id.clone());
    }
    let lookup = |v: &str| -> Option<String> {
        let v = v.trim();
        if v.contains(':') || v.contains('/') {
            return None;
        }
        names.get(stem_of(v)).cloned()
    };
    let paths: Vec<String> = out.rows.iter().filter(|r| r.kind == "doc").map(|r| r.path.clone()).collect();
    for path in paths {
        edit_frontmatter(&mut out.plan, &path, |fm| {
            let Some(Value::Object(rel)) = fm.get_mut("rel") else { return };
            for (key, v) in rel.iter_mut() {
                if !crate::graph::DOC_RELATIONS.contains(&key.as_str()) {
                    continue;
                }
                match v {
                    Value::String(t) => {
                        if let Some(id) = lookup(t) {
                            *t = id;
                        }
                    }
                    Value::Array(items) => {
                        for item in items.iter_mut() {
                            if let Value::String(t) = item
                                && let Some(id) = lookup(t)
                            {
                                *t = id;
                            }
                        }
                    }
                    _ => {}
                }
            }
        });
    }
}

/// Changes the frontmatter of a document this plan writes.
fn edit_frontmatter(plan: &mut Plan, path: &str, f: impl FnOnce(&mut Map<String, Value>)) {
    for c in plan.changes.iter_mut() {
        if let crate::authoring::Change::Write { path: p, content, .. } = c
            && p == path
        {
            let split = frontmatter::split(content);
            let frontmatter::Parsed::Map(mut fm) = frontmatter::parse(split.raw) else { return };
            f(&mut fm);
            let body = split.body.to_string();
            *content = format!("{}{}", frontmatter::render(&crate::markdown::frontmatter::edit::ordered(fm)), body);
            return;
        }
    }
}

/// `a/b/c.md` relative to directory `a/x` → `../b/c.md`.
pub(super) fn relative(from_dir: &str, to: &str) -> String {
    let f: Vec<&str> = from_dir.split('/').filter(|s| !s.is_empty()).collect();
    let t: Vec<&str> = to.split('/').filter(|s| !s.is_empty()).collect();
    let common = f.iter().zip(&t).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<&str> = std::iter::repeat_n("..", f.len() - common).collect();
    parts.extend(&t[common..]);
    parts.join("/")
}

/// `docs/adr` + `../x/./y.md` → `docs/x/y.md`; None when it climbs above the root.
pub(super) fn join(dir: &str, link: &str) -> Option<String> {
    let mut parts: Vec<&str> = dir.split('/').filter(|s| !s.is_empty()).collect();
    for seg in link.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    Some(parts.join("/"))
}

fn dir_of(p: &str) -> &str {
    p.rsplit_once('/').map(|x| x.0).unwrap_or("")
}

/// Relative links in imported bodies that named old files now point at the new paths; links to
/// files outside the import stay for L1/L2 to report.
pub(super) fn rewrite_links(out: &mut ImportPlan, by_source: &BTreeMap<String, String>) {
    let paths: BTreeMap<String, String> =
        out.rows.iter().filter(|r| r.kind == "doc").map(|r| (r.id.clone(), r.path.clone())).collect();
    for row in out.rows.iter_mut().filter(|r| r.kind == "doc" && !r.old_main.is_empty()) {
        let old_dir = dir_of(&row.old_main).to_string();
        let new_dir = dir_of(&row.path).to_string();
        let mut count = 0;
        for c in out.plan.changes.iter_mut() {
            let crate::authoring::Change::Write { path, content, .. } = c else { continue };
            if *path != row.path {
                continue;
            }
            // C180: inline, <angle>, reference-definition and percent-encoded links alike;
            // fenced and inline code stay as written.
            let (text, n) = crate::markdown::link_dest::rewrite(content, |target| {
                (!target.contains("://") && !target.starts_with('/'))
                    .then(|| join(&old_dir, target))
                    .flatten()
                    .and_then(|old| by_source.get(&old))
                    .and_then(|id| paths.get(id))
                    .map(|p| relative(&new_dir, p))
            });
            *content = text;
            count += n;
        }
        if count > 0 {
            row.notes.push(format!("{count} link(s) to old file names now point at the new paths"));
        }
    }
}

/// The body without the `## <name>` section (its heading up to the next heading).
pub(super) fn without_section(body: &str, name: &str) -> Option<String> {
    let is_it = |l: &str| l.starts_with('#') && l.trim_start_matches('#').trim().eq_ignore_ascii_case(name);
    let lines: Vec<&str> = body.split_inclusive('\n').collect();
    let at = lines.iter().position(|l| is_it(l.trim()))?;
    let end = lines[at + 1..].iter().position(|l| l.starts_with('#')).map_or(lines.len(), |e| at + 1 + e);
    Some([&lines[..at], &lines[end..]].concat().concat())
}
