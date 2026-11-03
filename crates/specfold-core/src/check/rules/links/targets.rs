//! How link findings name their targets: document ids, and references into remote repositories.

use crate::check::context::Ctx;
use crate::check::finding::Finding;

pub(super) fn ids(ctx: &Ctx, list: &[usize]) -> Vec<String> {
    list.iter().map(|&i| ctx.corpus.docs[i].id.to_string()).collect()
}

/// A `<remote>:<id>` reference: `None` when it is not one, `Some(None)` when it resolves (or
/// the remote's directory is absent, which is not reported), `Some(Some(problem))` otherwise.
/// With a relation, the target's type is checked as for local targets.
pub(super) fn remote_problem(ctx: &Ctx, text: &str, rel: Option<(&str, &str)>) -> Option<Option<String>> {
    use crate::remotes::Remote;
    Some(match crate::remotes::resolve(&ctx.corpus.cfg, text) {
        Remote::Local => return None,
        Remote::Absent { .. } => None,
        Remote::Found { ty, .. } => match rel.map(|(kind, own)| (allowed_targets(kind), kind, own)) {
            Some((Some([]), kind, own)) if ty != own => Some(format!("is a {ty}; rel.{kind} needs a {own}")),
            Some((Some(list), kind, _)) if !list.is_empty() && !list.contains(&ty.as_str()) => {
                Some(format!("is a {ty}, which rel.{kind} cannot point at"))
            }
            _ => None,
        },
        Remote::Missing { name, id } => Some(format!("names no document in the remote {name} ({id})")),
        Remote::MissingAnchor { name, id, anchor } => Some(format!("{name}:{id} has no anchor #{anchor}")),
        Remote::Ambiguous { name, ids } => Some(format!("is ambiguous in the remote {name}: {}", ids.join(", "))),
    })
}

/// F2 (`[check] remote_status`): a relation that names a remote document retired there.
pub(super) fn remote_status(
    ctx: &Ctx,
    d: &crate::model::Doc,
    line: usize,
    kind: &str,
    t: &str,
    out: &mut Vec<Finding>,
) {
    if !ctx.corpus.cfg.raw.check.remote_status || kind == "supersedes" {
        return;
    }
    let Some((status, by)) = crate::remotes::status(&ctx.corpus.cfg, t) else { return };
    if !matches!(status.as_str(), "superseded" | "archived" | "rejected") {
        return;
    }
    let next = match by {
        Some(new) => format!("next: spec set {} rel.{kind}-={t} rel.{kind}+={new} --write", d.id),
        None => format!("next: spec set {} rel.{kind}-={t} --write", d.id),
    };
    out.push(Finding::warn(
        "L3",
        &d.path,
        line,
        format!("rel.{kind} target {t} is {status} in its repository; {next}"),
    ));
}

/// The types a relation may point at; `None` means any type, `Some(&[])` means the same type.
pub(super) fn allowed_targets(kind: &str) -> Option<&'static [&'static str]> {
    match kind {
        "supersedes" => Some(&[]),
        "implements" => Some(&["sot", "adr", "page", "design"]),
        "evidence" => Some(&["evidence", "research", "audit"]),
        _ => None,
    }
}
