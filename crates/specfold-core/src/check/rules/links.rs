//! L rules: every reference resolves to one document, written as an id.

use super::support::{key_line, quote, skipped};
use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::graph::{DOC_RELATIONS, Resolution};
use crate::model::RefText;

pub struct LinksResolve;
pub struct NoPathCites;
pub struct RelationsResolve;
pub struct AnchorsUnique;
pub struct GovernsExists;

fn ids(ctx: &Ctx, list: &[usize]) -> Vec<String> {
    list.iter().map(|&i| ctx.corpus.docs[i].id.to_string()).collect()
}

impl Rule for LinksResolve {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "L1",
            title: "every [[reference]] resolves to one document",
            explain: "Body references are written [[<id>]] or [[<id>#<anchor>]]. A reference that names nothing, an anchor that does not exist, or an alias shared by two documents is an error. An alias that resolves is accepted but reported, with the id to write instead; `spec fix --rule L1 --write` rewrites them all.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers_links(i) || skipped(d, "L1") {
                continue;
            }
            for l in &d.links {
                match ctx.graph.resolve(&l.item) {
                    Resolution::Doc { idx, anchor, via_alias: true } => {
                        let mut id = ctx.corpus.docs[idx].id.to_string();
                        if let Some(a) = anchor {
                            id = format!("{id}#{a}");
                        }
                        out.push(
                            Finding::warn("L1", &l.path, l.line, format!("[[{}]] is an alias of {id}", l.item))
                                .fix_replace(format!("[[{}]]", l.item), format!("[[{id}]]")),
                        );
                    }
                    Resolution::Doc { .. } => {}
                    Resolution::Ambiguous(list) => {
                        let c = ids(ctx, &list);
                        out.push(
                            Finding::error(
                                "L1",
                                &l.path,
                                l.line,
                                format!("[[{}]] is ambiguous ({} documents)", l.item, c.len()),
                            )
                            .fix_cmd(format!(
                                "spec fix --rule L1 --pick {} --write {}:{}",
                                c[0],
                                quote(&l.path),
                                l.line
                            ))
                            .with_candidates(c),
                        );
                    }
                    Resolution::MissingAnchor { idx, anchor } => {
                        let have: Vec<String> = ctx.graph.anchors[idx].iter().cloned().collect();
                        out.push(
                            Finding::error(
                                "L1",
                                &l.path,
                                l.line,
                                format!("{} has no anchor #{anchor}", ctx.corpus.docs[idx].id),
                            )
                            .with_candidates(have),
                        );
                    }
                    Resolution::Missing => {
                        let target = RefText::parse(&l.item).target;
                        out.push(
                            Finding::error("L1", &l.path, l.line, format!("[[{}]] names no document", l.item))
                                .fix_cmd(format!("spec find {}", quote(&target))),
                        );
                    }
                }
            }
        }
    }
}

impl Rule for NoPathCites {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "L2",
            title: "documents are cited by id, not by path",
            explain: "A path breaks when a document moves; an id does not. Plain-text paths into a spec directory are reported with the id to write instead.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers(i) || skipped(d, "L2") {
                continue;
            }
            for c in &d.path_cites {
                let f = Finding::error("L2", &c.path, c.line, format!("cites the path {}", c.item));
                out.push(match ctx.graph.resolve(&c.item) {
                    Resolution::Doc { idx, .. } => {
                        f.fix_replace(c.item.clone(), format!("[[{}]]", ctx.corpus.docs[idx].id))
                    }
                    _ => f,
                });
            }
        }
    }
}

/// The types a relation may point at; `None` means any type, `Some(&[])` means the same type.
fn allowed_targets(kind: &str) -> Option<&'static [&'static str]> {
    match kind {
        "supersedes" => Some(&[]),
        "implements" => Some(&["sot", "adr", "page", "design"]),
        "evidence" => Some(&["evidence", "research", "audit"]),
        _ => None,
    }
}

impl Rule for RelationsResolve {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "L3",
            title: "relations resolve and point at the right types",
            explain: "rel.supersedes names documents of the same type, rel.implements names rules or decisions (sot, adr, page, design), rel.evidence names evidence, research or audits, rel.depends_on names anything, and rel.governs names code paths. Other relation names are reported.",
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
                        _ => out.push(Finding::error(
                            "L3",
                            &d.path,
                            line,
                            format!("rel.{kind} target {t} names no document"),
                        )),
                    }
                }
            }
        }
    }
}

impl Rule for AnchorsUnique {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "L4",
            title: "anchors are unique within a document",
            explain: "Headings carry stable anchors as `## Title {#ANCHOR}`; references point at them with [[<id>#ANCHOR]]. One anchor per name per document, across its chapters.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers(i) {
                continue;
            }
            let mut seen = std::collections::HashSet::new();
            for a in &d.anchors {
                if !seen.insert(&a.item) {
                    out.push(Finding::error("L4", &a.path, a.line, format!("anchor #{} is defined twice", a.item)));
                }
            }
        }
    }
}

impl Rule for GovernsExists {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "L5",
            title: "governed paths exist",
            explain: "rel.governs names the code a document rules. When that code moves or is deleted the document is orphaned; update governs, move the document with `spec mv`, or archive it.",
            on_commit: false,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers(i) || skipped(d, "L5") {
                continue;
            }
            for g in d.rel().remove("governs").unwrap_or_default() {
                let base = g.split(['*', '?', '[', '{']).next().unwrap_or("").trim_end_matches('/');
                let base = if g.contains(['*', '?', '[', '{']) {
                    base.rsplit_once('/').map(|x| x.0).unwrap_or(base)
                } else {
                    base
                };
                if !base.is_empty() && !ctx.corpus.cfg.root.join(base).exists() {
                    out.push(Finding::warn(
                        "L5",
                        &d.path,
                        key_line(ctx, &d.path, "rel"),
                        format!("governs '{g}' but {base} does not exist"),
                    ));
                }
            }
        }
    }
}
