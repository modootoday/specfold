//! L1: every [[reference]] resolves to one document.

use super::targets::{ids, remote_problem};
use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::check::rules::support::{quote, skipped};
use crate::graph::Resolution;
use crate::model::RefText;

pub struct LinksResolve;

/// The external root (an agent's memory, personal notes) holding a note named `target`.
fn external_note(ctx: &Ctx, target: &str) -> Option<String> {
    let cfg = &ctx.corpus.cfg;
    cfg.raw
        .external
        .iter()
        .find(|(_, ext)| cfg.expand_path(&ext.path).join(format!("{target}.md")).is_file())
        .map(|(name, _)| name.clone())
}

impl Rule for LinksResolve {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "L1",
            title: "every [[reference]] resolves to one document",
            explain: "Body references are written [[<id>]] or [[<id>#<anchor>]]. A reference that names nothing, an anchor that does not exist, or an alias shared by two documents is an error. A Markdown link whose target is a document id, `[text](<id>)`, is reported with the [[<id>|text]] to write instead (`spec fix --rule L1 --write` rewrites it); the built-in export rewrites the ones that resolve. An alias that resolves is accepted but reported, with the id to write instead; `spec fix --rule L1 --write` rewrites them all. [[<remote>:<id>]] names a document in a repository listed in [remotes]; it is checked when that directory is on disk and skipped when it is not.",
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
                        let how = if ctx.graph.by_slug.contains_key(RefText::parse(&l.item).target.as_str()) {
                            "names by its slug"
                        } else {
                            "is an alias of"
                        };
                        out.push(
                            Finding::warn("L1", &l.path, l.line, format!("[[{}]] {how} {id}", l.item))
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
                                crate::shell::word(&c[0]),
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
                        if let Some(msg) = remote_problem(ctx, &l.item, None) {
                            if let Some(m) = msg {
                                out.push(Finding::error("L1", &l.path, l.line, format!("[[{}]] {m}", l.item)));
                            }
                            continue;
                        }
                        let target = RefText::parse(&l.item).target;
                        if let Some(root) = external_note(ctx, &target) {
                            out.push(Finding::warn(
                                "L1",
                                &l.path,
                                l.line,
                                format!(
                                    "[[{}]] is a note in the external root {root}, which other readers cannot open",
                                    l.item
                                ),
                            ));
                            continue;
                        }
                        out.push(
                            Finding::error("L1", &l.path, l.line, format!("[[{}]] names no document", l.item))
                                .fix_cmd(format!("spec find {}", quote(&target))),
                        );
                    }
                }
            }
            md_id_links(ctx, d, out);
        }
    }
}

/// `[text](plan:root/…)` is a Markdown link, not a reference: nothing checks it and a reader's
/// tool cannot open it. Warns and offers `[[id|text]]`.
fn md_id_links(ctx: &Ctx, d: &crate::model::Doc, out: &mut Vec<Finding>) {
    let files = std::iter::once(d.path.as_str()).chain(d.chapters.iter().map(String::as_str));
    for path in files {
        let Some(text) = ctx.current(path) else { continue };
        for o in crate::export::links::md_id_occurrences(&text) {
            let ty = o.target.split(':').next().unwrap_or_default();
            if !ctx.corpus.docs.iter().any(|x| x.id.ty == ty) {
                continue;
            }
            let line = text[..o.span.0].matches('\n').count() + 1;
            let resolved = ctx.graph.resolve(&o.target);
            // A slug id or alias is written as the canonical id, so the fix needs no second pass.
            let target = match &resolved {
                Resolution::Doc { idx, anchor, .. } => {
                    let id = ctx.corpus.docs[*idx].id.to_string();
                    anchor.as_ref().map_or(id.clone(), |a| format!("{id}#{a}"))
                }
                _ => o.target.clone(),
            };
            let wiki = match &o.label {
                Some(l) => format!("[[{target}|{l}]]"),
                None => format!("[[{target}]]"),
            };
            let f = match resolved {
                Resolution::Doc { .. } => Finding::warn(
                    "L1",
                    path,
                    line,
                    format!("{} is a Markdown link to a document; write {wiki} so check and export follow it", o.raw),
                )
                .fix_replace(o.raw.clone(), wiki),
                _ => Finding::warn(
                    "L1",
                    path,
                    line,
                    format!(
                        "{} is a Markdown link to a document id that names no document; write {wiki} with the id from spec find",
                        o.raw
                    ),
                ),
            };
            out.push(f);
        }
    }
}
