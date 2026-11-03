//! Checks on values typed on the command line: types, statuses, dates and line breaks.

use super::Session;
use anyhow::{Result, bail};

/// C160/C162: characters that end a line, YAML 1.1 included (NEL, LS, PS).
pub const LINE_BREAKS: [char; 5] = ['\n', '\r', '\u{85}', '\u{2028}', '\u{2029}'];

/// C148/C153: a filter value no document can have is a typo, not an empty result.
pub fn known_types(s: &Session, given: &[String]) -> Result<()> {
    let names = s.corpus.cfg.registry.names();
    if let Some(t) = given.iter().find(|t| !names.contains(&t.as_str())) {
        bail!("unknown type '{t}' (have: {})", names.join(", "));
    }
    Ok(())
}

/// See [`known_types`]: every status some type allows.
pub fn known_statuses(s: &Session, given: &[String]) -> Result<()> {
    let reg = &s.corpus.cfg.registry;
    let mut all: Vec<&str> =
        reg.names().iter().filter_map(|n| reg.get(n)).flat_map(|t| t.statuses.iter().map(|x| x.as_str())).collect();
    all.sort();
    all.dedup();
    if let Some(v) = given.iter().find(|v| !all.contains(&v.as_str())) {
        bail!("unknown status '{v}' (have: {})", all.join(", "));
    }
    Ok(())
}

/// C148: a real calendar date written YYYY-MM-DD.
pub fn check_date(flag: &str, d: &str) -> Result<()> {
    let p: Vec<&str> = d.split('-').collect();
    let ok = match p.as_slice() {
        [y, m, dd] if y.len() == 4 && m.len() == 2 && dd.len() == 2 => {
            match (y.parse::<u32>(), m.parse::<u32>(), dd.parse::<u32>()) {
                (Ok(y), Ok(m), Ok(dd)) => {
                    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
                    let days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
                    (1..=12).contains(&m) && dd >= 1 && dd <= days[m as usize - 1]
                }
                _ => false,
            }
        }
        _ => false,
    };
    if !ok {
        bail!("{flag} {d}: not a date; write it as YYYY-MM-DD, e.g. 2026-01-05");
    }
    Ok(())
}
