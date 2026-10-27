//! UTC timestamps used as document keys, and ISO dates used in frontmatter.

use time::{OffsetDateTime, format_description::FormatItem, macros::format_description};

const STAMP: &[FormatItem<'static>] = format_description!("[year][month][day][hour][minute][second]");
const DATE: &[FormatItem<'static>] = format_description!("[year]-[month]-[day]");

pub fn now_stamp() -> String {
    OffsetDateTime::now_utc().format(STAMP).unwrap_or_default()
}

pub fn today() -> String {
    OffsetDateTime::now_utc().format(DATE).unwrap_or_default()
}

/// The stamp one second after `stamp`, used when two documents would share a key.
pub fn next_stamp(stamp: &str) -> Option<String> {
    let parsed = time::PrimitiveDateTime::parse(stamp, STAMP).ok()?;
    (parsed + time::Duration::seconds(1)).format(STAMP).ok()
}

pub fn is_stamp(s: &str) -> bool {
    s.len() == 14 && s.bytes().all(|b| b.is_ascii_digit()) && time::PrimitiveDateTime::parse(s, STAMP).is_ok()
}

pub fn is_date(s: &str) -> bool {
    time::Date::parse(s, DATE).is_ok()
}

/// Accepts `YYYYMMDD`, `YYYYMMDDHHMMSS` and ISO dates; returns the ISO date.
pub fn normalize_date(s: &str) -> Option<String> {
    let s = s.trim().trim_matches('"').trim_matches('\'');
    if is_date(s) {
        return Some(s.to_string());
    }
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.len() >= 8 {
        let d = format!("{}-{}-{}", &digits[0..4], &digits[4..6], &digits[6..8]);
        if is_date(&d) {
            return Some(d);
        }
    }
    if s.len() >= 10 && is_date(&s[0..10]) {
        return Some(s[0..10].to_string());
    }
    None
}

pub fn is_slug(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 80
        && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !s.starts_with('-')
        && !s.ends_with('-')
        && !s.contains("--")
}

/// Turns a title into a slug: ASCII letters and digits, words joined by `-`, at most 60 bytes.
pub fn slugify(title: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if !out.is_empty() && !dash {
            out.push('-');
            dash = true;
        }
    }
    let mut s = out.trim_matches('-').to_string();
    if s.len() > 60 {
        s.truncate(60);
        s = s.trim_end_matches('-').to_string();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_and_dates() {
        assert!(is_stamp("20261027073000"));
        assert!(!is_stamp("20261327073000"));
        assert_eq!(next_stamp("20261027235959").as_deref(), Some("20261028000000"));
        assert_eq!(normalize_date("20261027").as_deref(), Some("2026-10-27"));
        assert_eq!(normalize_date("\"2026-10-27\"").as_deref(), Some("2026-10-27"));
        assert_eq!(normalize_date("2026-10-27T07:00:00Z").as_deref(), Some("2026-10-27"));
    }

    #[test]
    fn slugs() {
        assert_eq!(
            slugify("Keep a general core, take behaviour by config!"),
            "keep-a-general-core-take-behaviour-by-config"
        );
        assert!(is_slug("guard-policy"));
        assert!(!is_slug("Guard"));
        assert!(!is_slug("a--b"));
    }
}
