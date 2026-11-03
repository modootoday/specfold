//! T5: exceptions are reviewed by their date.

use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::check::rules::support::key_line;
use crate::model::stamp;

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
