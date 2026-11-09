//! `[budgets]`: size limits per path glob, written in spec.toml or read from another file, and
//! what one file measures against them (rule B1).

use crate::config::Config;
use crate::config::types::BudgetLimit;
use crate::workspace::owners::{PathMatch, matcher};
use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One glob's limits and where they were read.
#[derive(Debug, Clone)]
pub struct Budget {
    pub glob: String,
    /// `spec.toml`, or the `<file>#<key>` of `[budgets] from`.
    pub source: String,
    pub limit: BudgetLimit,
    m: PathMatch,
}

impl Budget {
    pub fn matches(&self, path: &str) -> bool {
        self.m.is_match(path)
    }
}

/// What a file measures and the limits that apply to it, in the order they were read.
#[derive(Debug, Clone, Serialize)]
pub struct Measure {
    pub path: String,
    pub bytes: u64,
    pub lines: u64,
    pub limits: Vec<Applied>,
    /// Whether any limit is exceeded.
    pub over: bool,
}

/// One limit as applied to a file: the glob, its source, the maxima, and which were exceeded.
#[derive(Debug, Clone, Serialize)]
pub struct Applied {
    pub glob: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_lines: Option<u64>,
    pub over_bytes: bool,
    pub over_lines: bool,
}

/// The array form of a budget table: `[[x]] path = "…"` (or `paths = […]`), `bytes`, `lines`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    path: Option<String>,
    #[serde(default)]
    paths: Vec<String>,
    bytes: Option<u64>,
    lines: Option<u64>,
}

/// Every budget, spec.toml's first, then those of `[budgets] from`. Any source that cannot be
/// read, a table of the wrong shape, a bad glob or an entry with neither limit is an error.
pub fn load(cfg: &Config) -> Result<Vec<Budget>> {
    let sec = &cfg.raw.budgets;
    let mut out = vec![];
    for (glob, limit) in &sec.paths {
        out.push(budget(glob, "spec.toml", *limit)?);
    }
    if let Some(from) = &sec.from {
        let v = super::source::read(&cfg.spec_home, from).context("[budgets] from")?;
        out.extend(parse(&v, from).with_context(|| format!("[budgets] from = \"{from}\""))?);
    }
    Ok(out)
}

fn budget(glob: &str, source: &str, limit: BudgetLimit) -> Result<Budget> {
    if limit.bytes.is_none() && limit.lines.is_none() {
        bail!("{source}: the budget for '{glob}' sets neither bytes nor lines");
    }
    let m = matcher(glob).ok_or_else(|| anyhow!("{source}: '{glob}' is not a valid glob"))?;
    Ok(Budget { glob: glob.to_string(), source: source.to_string(), limit, m })
}

/// A table of glob → `{ bytes, lines }`, or an array of `{ path | paths, bytes, lines }`.
fn parse(v: &Value, source: &str) -> Result<Vec<Budget>> {
    let mut out = vec![];
    match v {
        Value::Object(map) => {
            for (glob, l) in map {
                let limit: BudgetLimit = serde_json::from_value(l.clone())
                    .with_context(|| format!("'{glob}' is not {{ bytes = <n>, lines = <n> }}"))?;
                out.push(budget(glob, source, limit)?);
            }
        }
        Value::Array(list) => {
            for (i, e) in list.iter().enumerate() {
                let e: Entry = serde_json::from_value(e.clone())
                    .with_context(|| format!("entry {} is not {{ path, bytes, lines }}", i + 1))?;
                let globs: Vec<String> = e.path.into_iter().chain(e.paths).collect();
                if globs.is_empty() {
                    bail!("entry {} names no path", i + 1);
                }
                for g in globs {
                    out.push(budget(&g, source, BudgetLimit { bytes: e.bytes, lines: e.lines })?);
                }
            }
        }
        _ => bail!("the budgets are neither a table nor an array"),
    }
    Ok(out)
}

/// `path` with content `text` measured against every budget whose glob matches it; None when
/// none does.
pub fn measure(budgets: &[Budget], path: &str, text: &[u8]) -> Option<Measure> {
    let applied: Vec<&Budget> = budgets.iter().filter(|b| b.matches(path)).collect();
    if applied.is_empty() {
        return None;
    }
    let bytes = text.len() as u64;
    let lines = text.split(|b| *b == b'\n').count() as u64 - u64::from(text.ends_with(b"\n") || text.is_empty());
    let limits: Vec<Applied> = applied
        .iter()
        .map(|b| Applied {
            glob: b.glob.clone(),
            source: b.source.clone(),
            max_bytes: b.limit.bytes,
            max_lines: b.limit.lines,
            over_bytes: b.limit.bytes.is_some_and(|m| bytes > m),
            over_lines: b.limit.lines.is_some_and(|m| lines > m),
        })
        .collect();
    let over = limits.iter().any(|a| a.over_bytes || a.over_lines);
    Some(Measure { path: path.to_string(), bytes, lines, limits, over })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(glob: &str, bytes: Option<u64>, lines: Option<u64>) -> Budget {
        budget(glob, "spec.toml", BudgetLimit { bytes, lines }).unwrap()
    }

    #[test]
    fn measures_bytes_and_lines_against_each_matching_budget() {
        let budgets = [b("AGENTS.md", Some(10), None), b("**/*.md", None, Some(2)), b("src", Some(1), None)];
        let m = measure(&budgets, "AGENTS.md", b"one\ntwo\nthree\n").unwrap();
        assert_eq!((m.bytes, m.lines, m.limits.len(), m.over), (14, 3, 2, true));
        assert!(m.limits[0].over_bytes && m.limits[1].over_lines);
        let m = measure(&budgets, "a.md", b"x\ny").unwrap();
        assert_eq!((m.lines, m.over), (2, false));
        assert!(measure(&budgets, "b.txt", b"").is_none());
    }

    #[test]
    fn parses_both_table_shapes_and_refuses_empty_entries() {
        let t = serde_json::json!({"a.md": {"bytes": 5}});
        assert_eq!(parse(&t, "x").unwrap()[0].glob, "a.md");
        let a = serde_json::json!([{"paths": ["a.md", "b.md"], "lines": 3}]);
        assert_eq!(parse(&a, "x").unwrap().len(), 2);
        assert!(parse(&serde_json::json!({"a.md": {}}), "x").is_err());
        assert!(parse(&serde_json::json!({"a.md": {"byte": 1}}), "x").is_err());
        assert!(parse(&serde_json::json!([{"bytes": 1}]), "x").is_err());
        assert!(parse(&serde_json::json!(3), "x").is_err());
    }
}
