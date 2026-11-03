//! L3: every relation names a document of a type it may point at.

use super::targets::{allowed_targets, ids, remote_problem, remote_status};
use crate::check::context::{Ctx, key_line};
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::corpus::graph::{DOC_RELATIONS, Resolution};

pub struct RelationsResolve;

impl Rule for RelationsResolve {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "L3",
            title: "relations resolve and point at the right types",
            explain: "rel.supersedes names documents of the same type, rel.implements names rules or decisions (sot, adr, page, design), rel.evidence names evidence, research or audits, rel.depends_on, rel.translation_of and rel.exception_of name anything, and rel.governs names code paths. A target written <remote>:<id> is looked up in that repository from [remotes], with the same type rules, and skipped when its directory is absent. Other relation names are reported.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers_links(i) {
                continue;
            }
            for (kind, targets) in d.rel() {
                if kind == "governs" {
                    continue;
                }
                let line = key_line(ctx, &d.path, "rel");
                if !DOC_RELATIONS.contains(&kind.as_str()) {
                    out.push(Finding::warn("L3", &d.path, line, format!("unknown relation rel.{kind}")));
                    continue;
                }
                for t in targets {
                    match ctx.graph.resolve(&t) {
                        Resolution::Doc { idx, .. } => {
                            let ty = &ctx.corpus.docs[idx].id.ty;
                            let ok = match allowed_targets(&kind) {
                                Some([]) => ty == &d.id.ty,
                                Some(list) => list.contains(&ty.as_str()),
                                None => true,
                            };
                            if !ok {
                                out.push(Finding::error(
                                    "L3",
                                    &d.path,
                                    line,
                                    format!("rel.{kind} cannot point at a {ty} ({t})"),
                                ));
                            }
                        }
                        Resolution::Ambiguous(list) => out.push(
                            Finding::error("L3", &d.path, line, format!("rel.{kind} target {t} is ambiguous"))
                                .with_candidates(ids(ctx, &list)),
                        ),
                        _ => match remote_problem(ctx, &t, Some((&kind, &d.id.ty))) {
                            Some(None) => remote_status(ctx, d, line, &kind, &t, out),
                            Some(Some(m)) => {
                                out.push(Finding::error("L3", &d.path, line, format!("rel.{kind} target {t} {m}")))
                            }
                            None => out.push(Finding::error(
                                "L3",
                                &d.path,
                                line,
                                format!("rel.{kind} target {t} names no document"),
                            )),
                        },
                    }
                }
            }
        }
    }
}
