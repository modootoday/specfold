//! S5: dates are dates, with a zone where they carry a time, and `updated` is not before `created`.

use crate::check::context::{Ctx, key_line};
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::model::stamp;

pub struct Dates;

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
