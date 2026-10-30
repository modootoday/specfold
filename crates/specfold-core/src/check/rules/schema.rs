//! S rules: every document is where its type says, and its frontmatter has the right shape.

use super::support::{COMMON_KEYS, REQUIRED_KEYS, key_line, nearest, quote};
use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::model::stamp;

pub struct Frontmatter;
pub struct IdMatchesPath;
pub struct Placement;
pub struct StatusAllowed;
pub struct Dates;
pub struct RequiredByStatus;

/// Whether `key` (or `rel.<kind>`) holds a non-empty value.
fn has_value(fm: &serde_json::Map<String, serde_json::Value>, key: &str) -> bool {
    let v = match key.strip_prefix("rel.") {
        Some(kind) => fm.get("rel").and_then(|r| r.get(kind)),
        None => fm.get(key),
    };
    match v {
        None | Some(serde_json::Value::Null) => false,
        Some(serde_json::Value::String(s)) => !s.trim().is_empty(),
        Some(serde_json::Value::Array(a)) => !a.is_empty(),
        Some(_) => true,
    }
}

impl Rule for RequiredByStatus {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "S6",
            title: "a status has the fields it requires",
            explain: "A type can require fields once a document reaches a status, through `require_when` in spec.toml (for example a review date for an accepted risk, or rel.depends_on for an active plan). `spec set <id> key=value` adds them.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers(i) {
                continue;
            }
            let (Some(ty), Some(fm), Some(status)) = (ctx.corpus.cfg.registry.get(&d.id.ty), d.fm(), d.status()) else {
                continue;
            };
            let Some(required) = ty.require_when.get(&status) else { continue };
            let missing: Vec<&str> = required.iter().map(String::as_str).filter(|k| !has_value(fm, k)).collect();
            if !missing.is_empty() {
                out.push(
                    Finding::error(
                        "S6",
                        &d.path,
                        key_line(ctx, &d.path, "status"),
                        format!("status '{status}' requires {}", missing.join(", ")),
                    )
                    .fix_cmd(format!(
                        "spec set {} {}",
                        crate::shell::word(&d.id),
                        missing.iter().map(|k| format!("{k}=…")).collect::<Vec<_>>().join(" ")
                    )),
                );
            }
        }
    }
}

impl Rule for Frontmatter {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "S1",
            title: "frontmatter parses and has the required keys",
            explain: "Every document starts with a YAML frontmatter block holding id, title, status, created and updated. Keys outside the common set and the type's own fields are reported so typos do not pass silently; a key that is meant to stay is declared in spec.toml as `[types.<type>] fields = [...]`, a list that replaces the type's built-in fields, so it keeps them, or for every type as `[check] extra_keys = [\"team\", \"review_*\"]` (a trailing * allows a prefix). `spec fix --rule S1` fills missing keys from the path and the first heading.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers(i) {
                continue;
            }
            if let Some(e) = &d.frontmatter.error
                && e.starts_with(crate::corpus::overlay::UTF8)
            {
                let line = crate::corpus::overlay::utf8_line(e).unwrap_or(1);
                out.push(Finding::error("S1", &d.path, line, format!("file is {e}; save it as UTF-8")));
                continue;
            }
            if let Some(e) = &d.frontmatter.error {
                out.push(Finding::error("S1", &d.path, 1, format!("frontmatter does not parse: {e}")));
                continue;
            }
            let Some(fm) = d.fm() else {
                out.push(
                    Finding::error("S1", &d.path, 1, "no frontmatter block")
                        .fix_cmd(format!("spec fix --rule S1 --write {}", quote(&d.path))),
                );
                continue;
            };
            let missing: Vec<&str> = REQUIRED_KEYS.iter().copied().filter(|k| !fm.contains_key(*k)).collect();
            if !missing.is_empty() {
                out.push(
                    Finding::error("S1", &d.path, 1, format!("missing {}", missing.join(", ")))
                        .fix_cmd(format!("spec fix --rule S1 --write {}", quote(&d.path))),
                );
            }
            let ty = ctx.corpus.cfg.registry.get(&d.id.ty);
            for k in fm.keys() {
                let known = COMMON_KEYS.contains(&k.as_str())
                    || ctx.corpus.cfg.raw.check.extra_key(k)
                    || ty.is_some_and(|t| t.fields.iter().any(|f| f == k) || (t.sealed && k == "sealed"));
                if !known {
                    out.push(Finding::warn(
                        "S1",
                        &d.path,
                        key_line(ctx, &d.path, k),
                        unknown_key_message(k, &d.id.ty, ty.map(|t| t.fields.as_slice()).unwrap_or_default()),
                    ));
                }
            }
        }
    }
}

/// The S1 warning `spec set` would cause by writing `key` on a document of type `ty`, so set can
/// say it at once.
pub fn unknown_key(c: &crate::corpus::Corpus, ty: &str, key: &str) -> Option<String> {
    let k = key.split('.').next().unwrap_or(key);
    let t = c.cfg.registry.get(ty);
    let known = COMMON_KEYS.contains(&k)
        || c.cfg.raw.check.extra_key(k)
        || t.is_some_and(|t| t.fields.iter().any(|f| f == k) || (t.sealed && k == "sealed"));
    (!known).then(|| unknown_key_message(k, ty, t.map(|t| t.fields.as_slice()).unwrap_or_default()))
}

/// Names a close known key when the unknown one looks like a typo, and otherwise shows the
/// spec.toml line that declares it. `fields` replaces the type's list, so the line keeps it.
fn unknown_key_message(k: &str, ty: &str, fields: &[String]) -> String {
    let known = COMMON_KEYS.iter().map(|s| s.to_string()).chain(fields.iter().cloned());
    if let Some(near) = known.filter(|c| c != k).find(|c| edits(c, k) <= 2.min(k.len() / 3)) {
        return format!("unknown key '{k}' for {ty}; did you mean '{near}'?");
    }
    let list: Vec<String> = fields.iter().map(String::as_str).chain([k]).map(|f| format!("\"{f}\"")).collect();
    format!(
        "unknown key '{k}' for {ty}; to keep it, declare it in spec.toml: [types.{ty}] fields = [{}], or for every type: [check] extra_keys = [\"{k}\"]",
        list.join(", ")
    )
}

fn edits(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for j in 0..b.len() {
            let cur = row[j + 1];
            row[j + 1] = (prev + usize::from(ca != b[j])).min(row[j] + 1).min(cur + 1);
            prev = cur;
        }
    }
    row[b.len()]
}

impl Rule for IdMatchesPath {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "S2",
            title: "id matches the path",
            explain: "A document's id is computed from where it lives: <type>:<owner>/<key>. The id written in frontmatter must equal it, so a copied or moved file cannot keep a stale id. Move documents with `spec mv`, which rewrites both.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers(i) {
                continue;
            }
            let Some(declared) = d.str_field("id") else { continue };
            let expected = d.id.to_string();
            if declared != expected {
                let line = key_line(ctx, &d.path, "id");
                out.push(
                    Finding::error(
                        "S2",
                        &d.path,
                        line,
                        format!("id is '{declared}' but the path makes it '{expected}'"),
                    )
                    .fix_replace(format!("id: {declared}"), format!("id: {expected}")),
                );
            }
        }
    }
}

impl Rule for Placement {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "S3",
            title: "every file in a spec directory belongs to a document",
            explain: "A spec directory holds only type directories, and in them only `<key>-<slug>.<type>.md` files or document bundles. Anything else (old layouts, notes without a type, loose attachments) is reported. Create documents with `spec new`, attach files with `spec attach`, bring old layouts in with `spec import`.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for s in &ctx.corpus.strays {
            if ctx.scope.covers_path(&s.path) {
                out.push(Finding::error("S3", &s.path, 1, s.reason.clone()));
            }
        }
    }
}

impl Rule for StatusAllowed {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "S4",
            title: "status is one the type allows",
            explain: "Statuses come from one lifecycle (draft, active, superseded, archived, rejected); each type allows a subset. Words from other tools are not accepted but named: accepted means active, proposed means draft, deprecated means superseded. Change status with `spec set <id> status=<value>`, which also checks the transition.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers(i) {
                continue;
            }
            let (Some(status), Some(ty)) = (d.status(), ctx.corpus.cfg.registry.get(&d.id.ty)) else { continue };
            if !ty.statuses.contains(&status) {
                let line = key_line(ctx, &d.path, "status");
                out.push(
                    Finding::error(
                        "S4",
                        &d.path,
                        line,
                        format!(
                            "status '{status}' is not allowed for {} (allowed: {}){}",
                            ty.name,
                            ty.statuses.join(", "),
                            status_hint(&status, &ty.statuses)
                        ),
                    )
                    .with_candidates(by_closeness(&status, &ty.statuses)),
                );
            }
        }
    }
}

impl Rule for Dates {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "S5",
            title: "dates are ISO and updated is not before created",
            explain: "created and updated are UTC calendar dates written YYYY-MM-DD. `spec set` and `spec touch` keep updated current.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers(i) {
                continue;
            }
            let created = d.str_field("created");
            let updated = d.str_field("updated");
            for (key, val) in [("created", &created), ("updated", &updated)] {
                let Some(v) = val else { continue };
                if !stamp::is_date_value(v) {
                    let line = key_line(ctx, &d.path, key);
                    let f = Finding::error(
                        "S5",
                        &d.path,
                        line,
                        match stamp::zone_problem(v) {
                            Some(why) => format!("{key} '{v}': {why}"),
                            None => {
                                format!("{key} '{v}' is not YYYY-MM-DD, RFC 3339 or 'YYYY-MM-DD HH:MM UTC|GMT|KST|JST'")
                            }
                        },
                    );
                    out.push(match stamp::normalize_date(v) {
                        Some(n) => f.fix_replace(format!("{key}: {v}"), format!("{key}: {n}")),
                        None => f,
                    });
                }
            }
            if let (Some(c), Some(u)) = (&created, &updated)
                && let (Some(cd), Some(ud)) = (stamp::utc_day(c), stamp::utc_day(u))
                && ud < cd
            {
                out.push(Finding::error(
                    "S5",
                    &d.path,
                    key_line(ctx, &d.path, "updated"),
                    format!("updated {u} is before created {c}"),
                ));
            }
        }
    }
}

/// A common word from other tools mapped to the status meant; a hint only, never accepted as-is.
fn status_hint(status: &str, allowed: &[String]) -> String {
    let meant = match status.to_ascii_lowercase().as_str() {
        "accepted" | "approved" | "adopted" | "done" | "implemented" | "current" => "active",
        "proposed" | "proposal" | "wip" | "open" | "pending" => "draft",
        "deprecated" | "replaced" => "superseded",
        "declined" => "rejected",
        "closed" | "obsolete" | "retired" => "archived",
        _ => {
            return match nearest(status, allowed.iter().map(String::as_str)) {
                Some(n) => format!("; did you mean '{n}'?"),
                None => String::new(),
            };
        }
    };
    if allowed.iter().any(|s| s == meant) { format!("; for '{status}' use '{meant}'") } else { String::new() }
}

/// The allowed statuses with a near miss of `status` (a typo) or the word it stands for first,
/// so a picker offers the likely one on top.
fn by_closeness(status: &str, allowed: &[String]) -> Vec<String> {
    let hint = status_hint(status, allowed);
    let first = nearest(status, allowed.iter().map(String::as_str))
        .map(str::to_string)
        .or_else(|| hint.rsplit('\'').nth(1).map(str::to_string));
    let mut out = allowed.to_vec();
    if let Some(f) = first
        && let Some(i) = out.iter().position(|s| *s == f)
    {
        let v = out.remove(i);
        out.insert(0, v);
    }
    out
}

#[cfg(test)]
mod unknown_key_tests {
    #[test]
    fn suggests_a_near_key_or_the_declaration() {
        let f = vec!["verify".to_string()];
        assert!(super::unknown_key_message("staus", "adr", &f).ends_with("did you mean 'status'?"));
        assert_eq!(
            super::unknown_key_message("domain", "adr", &f),
            "unknown key 'domain' for adr; to keep it, declare it in spec.toml: [types.adr] fields = [\"verify\", \"domain\"], or for every type: [check] extra_keys = [\"domain\"]"
        );
    }
}
