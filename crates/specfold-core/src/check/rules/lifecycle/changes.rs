//! T2, T3 and T4: what changed since the last commit: a status only moves forward, a sealed body
//! stays as it was, and an edited body moves its `updated` date.

use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::check::rules::support::key_line;
use crate::markdown::frontmatter::{self, Parsed};
use crate::model::stamp;
use sha2::{Digest, Sha256};

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

fn previous_status(ctx: &Ctx, path: &str) -> Option<String> {
    field_of(&ctx.previous(path)?, "status")
}

impl Rule for ForwardOnly {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "T2",
            title: "status only moves forward",
            explain: "draft → active → superseded or archived, draft → rejected, and draft → superseded once a newer document names it in rel.supersedes (rule T1), unless the type sets its own `transitions` in spec.toml. `spec explain status` lists every type's moves. A decision that comes back is a new document that supersedes the old one, so the record of the first decision stays intact.",
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
                        "spec new {} --from {} --title '…' --write",
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
