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

/// The current time as written into frontmatter: RFC 3339 in UTC, `2026-10-30T02:21:00Z`.
pub fn now_value() -> String {
    let t = OffsetDateTime::now_utc().replace_nanosecond(0).unwrap_or_else(|_| OffsetDateTime::now_utc());
    t.format(&time::format_description::well_known::Rfc3339).unwrap_or_default()
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

/// A frontmatter date as written: `YYYY-MM-DD`, or an RFC 3339 time with its offset
/// (`2026-10-30T02:21:00Z`, `...+09:00`). specfold writes the plain date; both are read.
pub fn is_date_value(s: &str) -> bool {
    is_date(s) || instant(s).is_some()
}

/// Zone abbreviations read in `YYYY-MM-DD HH:MM[:SS] ABBR`. Only ones with a single fixed
/// meaning; CST, IST, BST and the like name several zones and are refused.
const ZONES: &[(&str, i8)] = &[("UTC", 0), ("GMT", 0), ("Z", 0), ("KST", 9), ("JST", 9)];
pub const AMBIGUOUS_ZONES: &[&str] = &["CST", "IST", "BST", "AST", "EST", "MST", "PST", "ECT", "SST"];

/// Why a date-like value with a zone abbreviation is refused, if it is.
pub fn zone_problem(s: &str) -> Option<String> {
    let abbr = s.rsplit(' ').next()?;
    AMBIGUOUS_ZONES.contains(&abbr).then(|| format!("zone '{abbr}' is ambiguous; write an offset such as +09:00 or Z"))
}

/// An RFC 3339 time, or `YYYY-MM-DD HH:MM[:SS] ABBR` with a known abbreviation.
fn instant(s: &str) -> Option<OffsetDateTime> {
    if let Ok(t) = OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339) {
        return Some(t);
    }
    let (at, abbr) = s.rsplit_once(' ')?;
    let hours = ZONES.iter().find(|(z, _)| *z == abbr)?.1;
    let p = time::PrimitiveDateTime::parse(at, format_description!("[year]-[month]-[day] [hour]:[minute]:[second]"))
        .or_else(|_| time::PrimitiveDateTime::parse(at, format_description!("[year]-[month]-[day] [hour]:[minute]")))
        .ok()?;
    Some(p.assume_offset(time::UtcOffset::from_hms(hours, 0, 0).ok()?))
}

/// The UTC calendar day of a frontmatter date, so ages and order compare like with like.
pub fn utc_day(s: &str) -> Option<String> {
    if is_date(s) {
        return Some(s.to_string());
    }
    instant(s)?.to_offset(time::UtcOffset::UTC).format(DATE).ok()
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
    #[test]
    fn reads_dates_with_an_offset_as_their_utc_day() {
        assert_eq!(super::utc_day("2026-10-30").as_deref(), Some("2026-10-30"));
        assert_eq!(super::utc_day("2026-10-30T02:21:00Z").as_deref(), Some("2026-10-30"));
        assert_eq!(super::utc_day("2026-10-30T02:21:00+09:00").as_deref(), Some("2026-10-29"));
        assert!(!super::is_date_value("2026-10-30 02:21"));
        assert_eq!(super::utc_day("2026-10-30 08:00 KST").as_deref(), Some("2026-10-29"));
        assert_eq!(super::utc_day("2026-10-30 08:00:05 UTC").as_deref(), Some("2026-10-30"));
        assert!(!super::is_date_value("2026-10-30 08:00 CST"));
        assert!(super::zone_problem("2026-10-30 08:00 CST").is_some());
    }

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
