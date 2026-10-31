//! T rules: supersede claims pair up, statuses only move forward, sealed bodies stay sealed,
//! an edited body moves its `updated` date, and exceptions are reviewed by their date.

use super::support::key_line;
use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::graph::query;
use crate::markdown::frontmatter::{self, Parsed};
use crate::model::stamp;
use sha2::{Digest, Sha256};

pub struct SupersedePairs;
pub struct ForwardOnly;
pub struct Sealed;
pub struct UpdatedMoves;

fn field_of(text: &str, key: &str) -> Option<String> {
    match frontmatter::parse(frontmatter::split(text).raw) {
        Parsed::Map(m) => m.get(key).and_then(|v| v.as_str()).map(str::to_string),
        _ => None,
    }
}

impl Rule for UpdatedMoves {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "T4",
            title: "an edited body moves its updated date",
            explain: "`updated` tells readers how current a document is. spec set, mv and attach keep it current, but a hand edit does not; this reports a commit whose body changed while `updated` stayed. `spec fix --rule T4 --write` sets it to today.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        if ctx.scope.is_all() {
            return;
        }
        let today = stamp::today();
        for &i in &ctx.scope.touched {
            let d = &ctx.corpus.docs[i];
            let (Some(before), Some(now)) = (ctx.previous(&d.path), ctx.current(&d.path)) else { continue };
            let (old_date, new_date) = (field_of(&before, "updated"), field_of(&now, "updated"));
            if body_hash(&before) == body_hash(&now) || old_date != new_date {
                continue;
            }
            let Some(date) = new_date.filter(|u| stamp::utc_day(u).as_deref() != Some(today.as_str())) else {
                continue;
            };
            out.push(
                Finding::warn(
                    "T4",
                    &d.path,
                    key_line(ctx, &d.path, "updated"),
                    format!("the body changed but updated is still {date}"),
                )
                .fix_replace(format!("updated: {date}"), format!("updated: {}", stamp::now_value())),
            );
        }
    }
}

impl Rule for SupersedePairs {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "T1",
            title: "superseded documents have exactly one successor",
            explain: "Supersession is written once, on the new document (rel.supersedes). The old document's status must then be superseded, and only one document may claim it. `spec new --from <id>` and `spec set <new> rel.supersedes+=<old> --write` keep both sides in step.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            // C204: a cross-type supersedes is an L3 error, not a successor.
            let successors: Vec<usize> = ctx
                .graph
                .edges_to(i)
                .filter(|e| e.kind == "supersedes" && ctx.corpus.docs[e.from].id.ty == d.id.ty)
                .map(|e| e.from)
                .collect();
            let covered = ctx.scope.covers(i) || successors.iter().any(|&s| ctx.scope.covers(s));
            if !covered {
                continue;
            }
            let status = d.status().unwrap_or_default();
            if let Some(cycle) = query::supersede_cycle(ctx.graph, i) {
                let ids: Vec<String> = cycle.iter().map(|&s| ctx.corpus.docs[s].id.to_string()).collect();
                out.push(
                    Finding::error(
                        "T1",
                        &d.path,
                        1,
                        format!("supersede cycle, nothing is current: {}", ids.join(" → ")),
                    )
                    .with_candidates(ids),
                );
            }
            if successors.len() > 1 {
                let c = successors.iter().map(|&s| ctx.corpus.docs[s].id.to_string()).collect();
                out.push(
                    Finding::error("T1", &d.path, 1, format!("{} documents supersede {}", successors.len(), d.id))
                        .with_candidates(c),
                );
            }
            if !successors.is_empty() && status != "superseded" && status != "archived" {
                out.push(
                    Finding::error(
                        "T1",
                        &d.path,
                        key_line(ctx, &d.path, "status"),
                        format!("superseded by {} but status is '{status}'", ctx.corpus.docs[successors[0]].id),
                    )
                    .fix_cmd(superseded_fix(ctx, d, &status, successors[0])),
                );
            }
            if successors.is_empty() && status == "superseded" {
                out.push(Finding::error(
                    "T1",
                    &d.path,
                    key_line(ctx, &d.path, "status"),
                    "status is superseded but no document supersedes it",
                ));
            }
        }
    }
}

/// C204: the command that marks `d` superseded; through a middle status when the lifecycle
/// has no direct move (draft), else dropping the relation on the successor.
fn superseded_fix(ctx: &Ctx, d: &crate::model::Doc, status: &str, successor: usize) -> String {
    let me = crate::shell::word(&d.id);
    let Some(ty) = ctx.corpus.cfg.registry.get(&d.id.ty) else {
        return format!("spec set {me} status=superseded --write");
    };
    if ty.transition_allowed(status, "superseded") {
        return format!("spec set {me} status=superseded --write");
    }
    match ty.statuses.iter().find(|m| ty.transition_allowed(status, m) && ty.transition_allowed(m, "superseded")) {
        Some(m) => format!("spec set {me} status={m} --write && spec set {me} status=superseded --write"),
        None => {
            format!("spec set {} rel.supersedes-={} --write", crate::shell::word(&ctx.corpus.docs[successor].id), d.id)
        }
    }
}

fn previous_status(ctx: &Ctx, path: &str) -> Option<String> {
    field_of(&ctx.previous(path)?, "status")
}

impl Rule for ForwardOnly {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "T2",
            title: "status only moves forward",
            explain: "draft → active → superseded or archived, and draft → rejected, unless the type sets its own `transitions` in spec.toml. A decision that comes back is a new document that supersedes the old one, so the record of the first decision stays intact.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        if ctx.scope.is_all() {
            return;
        }
        for &i in &ctx.scope.touched {
            let d = &ctx.corpus.docs[i];
            let (Some(before), Some(now)) = (previous_status(ctx, &d.path), d.status()) else { continue };
            let Some(ty) = ctx.corpus.cfg.registry.get(&d.id.ty) else { continue };
            if !ty.transition_allowed(&before, &now) {
                out.push(
                    Finding::error(
                        "T2",
                        &d.path,
                        key_line(ctx, &d.path, "status"),
                        format!("status moved back from '{before}' to '{now}'"),
                    )
                    .fix_cmd(format!(
                        "spec new {} --from {} --title '…'",
                        d.id.ty,
                        crate::shell::word(&d.id)
                    )),
                );
            }
        }
    }
}

fn body_hash(text: &str) -> String {
    let body = frontmatter::split(text).body;
    Sha256::digest(body.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

impl Rule for Sealed {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "T3",
            title: "sealed documents keep their body",
            explain: "Types marked sealed (audits by default) are snapshots: after the first commit only frontmatter may change. Record a new audit instead of editing an old one.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        if ctx.scope.is_all() {
            return;
        }
        for &i in &ctx.scope.touched {
            let d = &ctx.corpus.docs[i];
            if !ctx.corpus.cfg.registry.get(&d.id.ty).is_some_and(|t| t.sealed) {
                continue;
            }
            let (Some(before), Some(now)) = (ctx.previous(&d.path), ctx.current(&d.path)) else { continue };
            if body_hash(&before) != body_hash(&now) {
                out.push(Finding::error("T3", &d.path, 1, format!("{} is sealed; its body changed", d.id)));
            }
        }
    }
}

pub struct ExceptionsReviewed;

/// Today's date (UTC) as `YYYY-MM-DD`.
fn today() -> String {
    stamp::now_value().chars().take(10).collect()
}

/// Whether `s` is a calendar date written `YYYY-MM-DD`.
pub fn is_date(s: &str) -> bool {
    let mut it = s.split('-');
    let (Some(y), Some(m), Some(d), None) = (it.next(), it.next(), it.next(), it.next()) else { return false };
    if y.len() != 4 || m.len() != 2 || d.len() != 2 {
        return false;
    }
    match (y.parse::<i32>(), m.parse::<u8>().ok().and_then(|m| time::Month::try_from(m).ok()), d.parse::<u8>()) {
        (Ok(y), Some(m), Ok(d)) => time::Date::from_calendar_date(y, m, d).is_ok(),
        _ => false,
    }
}

impl Rule for ExceptionsReviewed {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "T5",
            title: "exceptions are reviewed by their date",
            explain: "A document that makes a local exception to another names it with rel.exception_of and sets review_by: YYYY-MM-DD, the date by which someone decides whether the exception still holds. T5 warns when review_by is missing or not a date, and when the date has passed (UTC). Renew it with `spec set <id> review_by=<date> --write`, or retire the exception (status rejected, or superseded by a newer exception) once the rule it departs from is kept again. An exception that names itself is a warning. The target must resolve like any relation (L3).",
            on_commit: false,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        let today = today();
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers(i) || d.rel().get("exception_of").is_none_or(|l| l.is_empty()) {
                continue;
            }
            let status = d.status().unwrap_or_default();
            if ["superseded", "archived", "rejected"].contains(&status.as_str()) {
                continue;
            }
            let me = d.id.to_string();
            if d.rel().get("exception_of").is_some_and(|l| l.contains(&me)) {
                out.push(
                    Finding::warn(
                        "T5",
                        &d.path,
                        key_line(ctx, &d.path, "rel"),
                        "rel.exception_of names this document itself; name the rule or decision it departs from",
                    )
                    .fix_cmd(format!(
                        "spec set {} rel.exception_of-={} --write",
                        crate::shell::word(&me),
                        crate::shell::word(&me)
                    )),
                );
            }
            let line = key_line(ctx, &d.path, "review_by");
            let fix = format!("spec set {} review_by=<YYYY-MM-DD> --write", crate::shell::word(&d.id));
            match d.str_field("review_by") {
                None => out.push(
                    Finding::warn("T5", &d.path, 1, "an exception (rel.exception_of) needs review_by: YYYY-MM-DD")
                        .fix_cmd(fix),
                ),
                Some(r) if !is_date(&r) => out.push(
                    Finding::warn("T5", &d.path, line, format!("review_by '{r}' is not a date (YYYY-MM-DD)"))
                        .fix_cmd(fix),
                ),
                Some(r) if r < today => out.push(
                    Finding::warn(
                        "T5",
                        &d.path,
                        line,
                        format!("exception was due for review on {r}: renew review_by, or retire the exception with status rejected or superseded"),
                    )
                    .fix_cmd(fix),
                ),
                Some(_) => {}
            }
        }
    }
}

/// Exceptions still in force whose `review_by` date has passed (UTC): index and date.
pub fn overdue_exceptions(c: &crate::corpus::Corpus) -> Vec<(usize, String)> {
    let today = today();
    c.docs
        .iter()
        .enumerate()
        .filter(|(_, d)| d.rel().get("exception_of").is_some_and(|l| !l.is_empty()))
        .filter(|(_, d)| !["superseded", "archived", "rejected"].contains(&d.status().unwrap_or_default().as_str()))
        .filter_map(|(i, d)| d.str_field("review_by").filter(|r| is_date(r) && *r < today).map(|r| (i, r)))
        .collect()
}
