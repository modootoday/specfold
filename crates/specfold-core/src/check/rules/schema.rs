//! S rules: every document is where its type says, and its frontmatter has the right shape.

use super::support::{COMMON_KEYS, REQUIRED_KEYS, key_line, quote};
use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::model::stamp;

pub struct Frontmatter;
pub struct IdMatchesPath;
pub struct Placement;
pub struct StatusAllowed;
pub struct Dates;

impl Rule for Frontmatter {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "S1",
            title: "frontmatter parses and has the required keys",
            explain: "Every document starts with a YAML frontmatter block holding id, title, status, created and updated. Keys outside the common set and the type's own fields are reported so typos do not pass silently. `spec fix --rule S1` fills missing keys from the path and the first heading.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers(i) {
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
                    || ty.is_some_and(|t| t.fields.iter().any(|f| f == k) || (t.sealed && k == "sealed"));
                if !known {
                    out.push(Finding::warn(
                        "S1",
                        &d.path,
                        key_line(ctx, &d.path, k),
                        format!("unknown key '{k}' for {}", d.id.ty),
                    ));
                }
            }
        }
    }
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
            explain: "Statuses come from one lifecycle (draft, active, superseded, archived, rejected); each type allows a subset. Change status with `spec set <id> status=<value>`, which also checks the transition.",
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
                            "status '{status}' is not allowed for {} (allowed: {})",
                            ty.name,
                            ty.statuses.join(", ")
                        ),
                    )
                    .with_candidates(ty.statuses.clone()),
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
                if !stamp::is_date(v) {
                    let line = key_line(ctx, &d.path, key);
                    let f = Finding::error("S5", &d.path, line, format!("{key} '{v}' is not YYYY-MM-DD"));
                    out.push(match stamp::normalize_date(v) {
                        Some(n) => f.fix_replace(format!("{key}: {v}"), format!("{key}: {n}")),
                        None => f,
                    });
                }
            }
            if let (Some(c), Some(u)) = (&created, &updated)
                && stamp::is_date(c)
                && stamp::is_date(u)
                && u < c
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
