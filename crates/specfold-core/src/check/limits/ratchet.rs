//! `[ratchet]`: warning counts per rule and owner, recorded in a committed file, that may only
//! fall. Rule W1 judges a full run against the record; `spec check --write-ratchet` lowers it.

use crate::check::finding::{Finding, Severity};
use crate::corpus::Corpus;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SCHEMA: &str = "spec.ratchet.v1";

/// Counts keyed rule, then owner.
pub type Counts = BTreeMap<String, BTreeMap<String, usize>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub schema: String,
    pub counts: Counts,
}

/// A full run held against the record.
#[derive(Debug, Clone, Default)]
pub struct Judged {
    /// The counts this run found, for the rules it ran.
    pub now: Counts,
    /// (rule, owner, recorded, now) where the count rose.
    pub rises: Vec<(String, String, usize, usize)>,
    /// (rule, owner, recorded, now) where it fell; `--write-ratchet` records them.
    pub falls: Vec<(String, String, usize, usize)>,
    /// Ratcheted rules that ran but have no entry in the record yet.
    pub unrecorded: Vec<String>,
}

/// The owner a finding counts for: the owner of its repository path, or `external.<name>` for a
/// finding in the external source `<name>:…`.
pub fn owner_of(c: &Corpus, path: &str) -> String {
    if let Some((name, _)) = path.split_once(':')
        && c.cfg.raw.external.contains_key(name)
    {
        return format!("external.{name}");
    }
    c.owners.containing(path).id.clone()
}

/// Warnings of the ratcheted rules among `ran`, per rule and owner; every ratcheted rule that
/// ran has an entry, empty when it found nothing.
pub fn count(c: &Corpus, findings: &[Finding], ran: &[&str]) -> Counts {
    let rules = &c.cfg.raw.ratchet.rules;
    let mut out: Counts =
        rules.iter().filter(|r| ran.contains(&r.as_str())).map(|r| (r.clone(), BTreeMap::new())).collect();
    for f in findings.iter().filter(|f| f.severity == Severity::Warn) {
        if let Some(per) = out.get_mut(&f.rule) {
            *per.entry(owner_of(c, &f.path)).or_default() += 1;
        }
    }
    out
}

pub fn judge(rec: &Record, now: Counts) -> Judged {
    let mut j = Judged::default();
    for (rule, per) in &now {
        let Some(was) = rec.counts.get(rule) else {
            j.unrecorded.push(rule.clone());
            continue;
        };
        let owners: std::collections::BTreeSet<&String> = was.keys().chain(per.keys()).collect();
        for o in owners {
            let (a, b) = (was.get(o).copied().unwrap_or(0), per.get(o).copied().unwrap_or(0));
            let entry = (rule.clone(), o.clone(), a, b);
            if b > a {
                j.rises.push(entry);
            } else if b < a {
                j.falls.push(entry);
            }
        }
    }
    j.now = now;
    j
}

/// The record lowered to this run: falls taken, rules not yet recorded added, rises kept as
/// recorded (raising a count is an edit of the file, made on purpose and reviewed).
pub fn lowered(rec: Option<&Record>, j: &Judged) -> Record {
    let mut counts = rec.map(|r| r.counts.clone()).unwrap_or_default();
    for rule in &j.unrecorded {
        counts.insert(rule.clone(), j.now.get(rule).cloned().unwrap_or_default());
    }
    for (rule, owner, _, now) in &j.falls {
        let per = counts.entry(rule.clone()).or_default();
        if *now == 0 {
            per.remove(owner);
        } else {
            per.insert(owner.clone(), *now);
        }
    }
    Record { schema: SCHEMA.into(), counts }
}

/// The record named by `[ratchet] file`; None when it does not exist yet.
pub fn load(c: &Corpus) -> Result<Option<Record>> {
    let Some(rel) = &c.cfg.raw.ratchet.file else { bail!("[ratchet] rules is set but [ratchet] file is not") };
    let path = c.cfg.spec_home.join(rel);
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path).with_context(|| format!("cannot read {rel}"))?;
    let r: Record = serde_json::from_str(&text).with_context(|| format!("{rel} is not a {SCHEMA} file"))?;
    if r.schema != SCHEMA {
        bail!("{rel} has schema {}, expected {SCHEMA}", r.schema);
    }
    Ok(Some(r))
}

pub fn write(c: &Corpus, r: &Record) -> Result<String> {
    let Some(rel) = c.cfg.raw.ratchet.file.clone() else { bail!("set [ratchet] file in spec.toml first") };
    let path = c.cfg.spec_home.join(&rel);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(r)? + "\n")?;
    Ok(rel)
}

/// W1's findings for a full run: an error per rise, a warning per rule not yet recorded, and an
/// error when the record cannot be read. Returns the judgement for the summary.
pub fn findings(c: &Corpus, findings: &[Finding], ran: &[&str], out: &mut Vec<Finding>) -> Option<Judged> {
    if c.cfg.raw.ratchet.rules.is_empty() {
        return None;
    }
    let file = c.cfg.raw.ratchet.file.clone().unwrap_or_else(|| "spec.toml".into());
    let rec = match load(c) {
        Ok(r) => r,
        Err(e) => {
            out.push(Finding::error("W1", &file, 1, format!("{e:#}; next: spec explain W1")));
            return None;
        }
    };
    let now = count(c, findings, ran);
    let empty = Record { schema: SCHEMA.into(), counts: Counts::new() };
    let j = judge(rec.as_ref().unwrap_or(&empty), now);
    for (rule, owner, was, now) in &j.rises {
        let msg =
            format!("{rule} warnings of {owner} rose from {was} to {now}; fix the new ones (the record only falls)");
        out.push(Finding::error("W1", &file, 1, msg).fix_cmd(format!("spec check --rule {rule}")));
    }
    if !j.unrecorded.is_empty() {
        let msg = format!("no recorded counts for {}; record them", j.unrecorded.join(", "));
        out.push(Finding::warn("W1", &file, 1, msg).fix_cmd("spec check --write-ratchet --yes"));
    }
    Some(j)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(v: &[(&str, &str, usize)]) -> Counts {
        let mut c = Counts::new();
        for (r, o, n) in v {
            c.entry(r.to_string()).or_default().insert(o.to_string(), *n);
        }
        c
    }

    #[test]
    fn rises_fail_falls_lower_and_new_rules_are_recorded_whole() {
        let rec = Record { schema: SCHEMA.into(), counts: counts(&[("L8", "shop", 3), ("L8", "blog", 2)]) };
        let mut now = counts(&[("L8", "shop", 4), ("V3", "ws", 1)]);
        now.get_mut("L8").unwrap().insert("ws".into(), 0);
        let j = judge(&rec, now);
        assert_eq!(j.rises, [("L8".into(), "shop".into(), 3, 4)]);
        assert_eq!(j.falls, [("L8".into(), "blog".into(), 2, 0)]);
        assert_eq!(j.unrecorded, ["V3"]);
        let low = lowered(Some(&rec), &j);
        assert_eq!(low.counts, counts(&[("L8", "shop", 3), ("V3", "ws", 1)]));
    }
}
