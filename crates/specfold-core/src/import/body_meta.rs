//! Status, date and links that older ADR layouts keep in the body instead of frontmatter:
//! adr-tools writes `Date: …` and a `## Status` section with `Supercedes [title](file)` lines;
//! MADR 2 writes `* Status: …` and `* Date: …` bullets under the title.

use super::map::BodyMap;
use regex::Regex;
use std::sync::LazyLock;

#[derive(Debug, Default, PartialEq)]
pub struct Found {
    pub status: Option<String>,
    pub date: Option<String>,
    /// (relation kind, target file or name as written)
    pub links: Vec<(String, String)>,
    /// Lines the map asked for and could not read, for the plan's review.
    pub missed: Vec<String>,
}

static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[[^\]]*\]\(([^)\s]+)\)").unwrap());

/// Lines outside fenced code, trimmed.
fn lines(body: &str) -> Vec<&str> {
    let mut fenced = false;
    let mut out = vec![];
    for l in body.lines() {
        let t = l.trim();
        if t.starts_with("```") || t.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if !fenced {
            out.push(t);
        }
    }
    out
}

/// `* Status: accepted` / `- Status: accepted` → `accepted`.
fn bullet<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let rest = line.strip_prefix("* ").or_else(|| line.strip_prefix("- "))?;
    labelled(rest, name)
}

/// `Date: 2016-02-12` → `2016-02-12`, case-insensitive on the label.
fn labelled<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let (label, value) = line.split_once(':')?;
    label.trim().eq_ignore_ascii_case(name).then(|| value.trim())
}

/// The first paragraph line of `## <name>`.
fn section<'a>(ls: &[&'a str], name: &str) -> Option<&'a str> {
    let at =
        ls.iter().position(|l| l.starts_with('#') && l.trim_start_matches('#').trim().eq_ignore_ascii_case(name))?;
    ls[at + 1..].iter().take_while(|l| !l.starts_with('#')).find(|l| !l.is_empty()).copied()
}

/// A value as a status: comments and template brackets go, a trailing link goes
/// (`superseded by [ADR-0005](0005-example.md)` → `superseded by`).
fn clean(v: &str) -> String {
    let v = v.split("<!--").next().unwrap_or(v);
    let v = LINK.replace_all(v, "");
    v.trim().trim_end_matches(['.', ',']).trim().to_string()
}

fn read(ls: &[&str], how: &str) -> Option<String> {
    let (kind, name) = how.split_once(':')?;
    let raw = match kind {
        "section" => section(ls, name),
        "bullet" => ls.iter().find_map(|l| bullet(l, name)),
        "line" => ls.iter().find_map(|l| labelled(l, name)),
        _ => None,
    }?;
    let v = clean(raw);
    (!v.is_empty()).then_some(v)
}

/// Whether `how` is a form [`read`] understands, for the map check.
pub fn valid(how: &str) -> bool {
    how.split_once(':').is_some_and(|(k, n)| ["section", "bullet", "line"].contains(&k) && !n.trim().is_empty())
}

pub fn scan(body: &str, map: &BodyMap) -> Found {
    let ls = lines(body);
    let mut f = Found::default();
    if let Some(how) = &map.status {
        f.status = read(&ls, how);
        if f.status.is_none() {
            f.missed.push(format!("no status in the body ({how})"));
        }
    }
    if let Some(how) = &map.date {
        f.date = read(&ls, how);
        if f.date.is_none() {
            f.missed.push(format!("no date in the body ({how})"));
        }
    }
    for l in &ls {
        let l = l.strip_prefix("* ").or_else(|| l.strip_prefix("- ")).unwrap_or(l);
        let l =
            l.split_once(':').filter(|(k, _)| k.trim().eq_ignore_ascii_case("status")).map(|x| x.1.trim()).unwrap_or(l);
        // The longest phrase wins, so "Superceded by" is not read as "Supercedes"-like prefixes.
        let mut phrases: Vec<(&String, &String)> = map.links.iter().collect();
        phrases.sort_by_key(|(p, _)| std::cmp::Reverse(p.len()));
        let lower = l.to_lowercase();
        let Some((phrase, kind)) = phrases.into_iter().find(|(p, _)| lower.starts_with(&p.to_lowercase())) else {
            continue;
        };
        if kind.as_str() == "-" {
            continue;
        }
        let targets: Vec<String> = LINK.captures_iter(&l[phrase.len()..]).map(|c| c[1].to_string()).collect();
        if targets.is_empty() {
            f.missed.push(format!("'{phrase}' line without a link: {l}"));
        }
        for t in targets {
            if !t.contains("://") {
                f.links.push((kind.clone(), t));
            }
        }
    }
    f
}

/// Who decided, who was consulted and who was informed, under one spelling each.
/// MADR 4 writes `decision-makers:` in frontmatter, MADR 3 `deciders:`, and MADR 2 and
/// Log4brains `* Deciders: …` bullets in the body. A frontmatter value wins over the body.
pub fn people(fm: &mut serde_json::Map<String, serde_json::Value>, body: &str) {
    use serde_json::Value;
    const FIELDS: [(&str, &[&str]); 3] = [
        ("decision_makers", &["decision_makers", "decision-makers", "deciders"]),
        ("consulted", &["consulted"]),
        ("informed", &["informed"]),
    ];
    let ls = lines(body);
    for (field, names) in FIELDS {
        let mut got: Option<Vec<String>> = None;
        for n in names {
            if let Some(v) = fm.remove(*n)
                && got.is_none()
            {
                got = Some(match v {
                    Value::Array(items) => items.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
                    Value::String(s) => split_people(&s),
                    _ => vec![],
                });
            }
        }
        if got.as_ref().is_none_or(Vec::is_empty) {
            let label = if field == "decision_makers" { ["deciders", "decision-makers"].as_slice() } else { &[field] };
            got = ls.iter().find_map(|l| label.iter().find_map(|n| bullet(l, n))).map(split_people);
        }
        let people: Vec<String> = got.unwrap_or_default().into_iter().filter(|p| !placeholder(p)).collect();
        if !people.is_empty() {
            fm.insert(field.into(), Value::Array(people.into_iter().map(Value::String).collect()));
        }
    }
}

fn split_people(v: &str) -> Vec<String> {
    let v = v.split("<!--").next().unwrap_or(v);
    v.split(',').map(|p| p.trim().trim_end_matches('.').trim().to_string()).filter(|p| !p.is_empty()).collect()
}

/// Template text left in place: `{list everyone involved}`, `[names]`.
fn placeholder(p: &str) -> bool {
    p.starts_with(['{', '[', '<'])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(toml: &str) -> BodyMap {
        toml::from_str(toml).unwrap()
    }

    #[test]
    fn people_from_frontmatter_or_body_bullets() {
        let mut fm = serde_json::Map::new();
        fm.insert("decision-makers".into(), "Ana, Bo".into());
        people(&mut fm, "* Deciders: ignored\n* Consulted: Cy <!-- optional -->\n- Informed: {list everyone}\n");
        assert_eq!(fm["decision_makers"], serde_json::json!(["Ana", "Bo"]));
        assert_eq!(fm["consulted"], serde_json::json!(["Cy"]));
        assert!(!fm.contains_key("decision-makers") && !fm.contains_key("informed"), "{fm:?}");
        let mut fm = serde_json::Map::new();
        people(&mut fm, "# T\n\n- Deciders: team\n```\n- Informed: x\n```\n");
        assert_eq!(serde_json::Value::Object(fm), serde_json::json!({"decision_makers": ["team"]}));
    }

    #[test]
    fn adr_tools_status_section_date_line_and_links() {
        let m = map(
            "status = \"section:Status\"\ndate = \"line:Date\"\n[links]\nSupercedes = \"supersedes\"\n\"Superceded by\" = \"-\"\n",
        );
        let body = "# 2. Second Record\n\nDate: 1992-01-12\n\n## Status\n\nAccepted\n\nSupercedes [1. First Record](0001-first-record.md)\n\n## Context\n";
        let f = scan(body, &m);
        assert_eq!(f.status.as_deref(), Some("Accepted"));
        assert_eq!(f.date.as_deref(), Some("1992-01-12"));
        assert_eq!(f.links, vec![("supersedes".to_string(), "0001-first-record.md".to_string())]);
        let old = "# 1. First\n\nDate: 1992-01-12\n\n## Status\n\nSuperceded by [2. Second Record](0002-second-record.md)\n\n## Context\n";
        let f = scan(old, &m);
        assert_eq!(f.status.as_deref(), Some("Superceded by"));
        assert!(f.links.is_empty() && f.missed.is_empty(), "{f:?}");
    }

    #[test]
    fn madr2_bullets_and_a_missing_value_is_reported() {
        let m = map("status = \"bullet:Status\"\ndate = \"bullet:Date\"\n[links]\n\"superseded by\" = \"-\"\n");
        let body = "# Use X\n\n* Status: superseded by [ADR-0005](0005-example.md) <!-- optional -->\n* Deciders: a, b\n\n## Context\n";
        let f = scan(body, &m);
        assert_eq!(f.status.as_deref(), Some("superseded by"));
        assert_eq!(f.date, None);
        assert_eq!(f.missed, vec!["no date in the body (bullet:Date)"]);
        assert!(valid("bullet:Status") && !valid("para:Status") && !valid("line:"));
    }
}
