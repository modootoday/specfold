//! Choosing the key and slug of an imported document.

use super::map::SourceRule;
use crate::model::doctype::{KeyKind, TypeDef};
use crate::model::stamp;
use crate::workspace::vcs;
use regex::Captures;
use std::path::Path;

/// Turns `20260101`, `20260101-120000` or `20260101120000` into a 14-digit stamp.
pub fn normalize_stamp(s: &str) -> Option<String> {
    let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    let full = match digits.len() {
        8 => format!("{digits}000000"),
        12 => format!("{digits}00"),
        14 => digits,
        _ => return None,
    };
    stamp::is_stamp(&full).then_some(full)
}

/// What is known about a source when its key and slug are chosen.
pub struct Source<'a> {
    pub path: &'a str,
    pub caps: Option<&'a Captures<'a>>,
    pub created: Option<&'a str>,
    pub title: &'a str,
    pub name_stem: &'a str,
}

pub fn choose(root: &Path, rule: &SourceRule, ty: &TypeDef, src: &Source) -> (String, String) {
    let Source { path, caps, created, title, name_stem } = *src;
    let cap = |g: &str| caps.and_then(|c| c.name(g)).map(|m| m.as_str().to_string());
    let slug = cap("slug")
        .map(|s| stamp::slugify(&s))
        .filter(|s| stamp::is_slug(s))
        .or_else(|| Some(stamp::slugify(title)).filter(|s| stamp::is_slug(s)))
        .or_else(|| Some(stamp::slugify(name_stem)).filter(|s| stamp::is_slug(s)))
        .unwrap_or_else(|| "untitled".into());
    if ty.key == KeyKind::Slug {
        return (slug.clone(), slug);
    }
    let from_name = || cap("key").and_then(|k| normalize_stamp(&k));
    let from_created = || created.and_then(normalize_stamp);
    let from_commit = || vcs::first_commit_stamp(root, path);
    let key = match rule.key.as_str() {
        "name" => from_name().or_else(from_commit).or_else(from_created),
        "created" => from_created().or_else(from_commit),
        _ => from_commit().or_else(from_name).or_else(from_created),
    }
    .unwrap_or_else(stamp::now_stamp);
    (key, slug)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps() {
        assert_eq!(normalize_stamp("20260101").as_deref(), Some("20260101000000"));
        assert_eq!(normalize_stamp("20260101-120000").as_deref(), Some("20260101120000"));
        assert_eq!(normalize_stamp("2026-01-01").as_deref(), Some("20260101000000"));
        assert!(normalize_stamp("ADR-001").is_none());
    }
}
