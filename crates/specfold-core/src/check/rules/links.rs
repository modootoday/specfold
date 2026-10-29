//! L rules: every reference resolves to one document, written as an id.

use super::support::{distance, key_line, quote, skipped};
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
        }
    }
}

impl Rule for NoPathCites {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "L2",
            title: "documents are cited by id, not by path",
            explain: "A path breaks when a document moves; an id does not. Plain-text paths into a spec directory are reported with the id to write instead; a stale path (after a move or an import) is matched by file name or slug, and `spec fix --rule L2 --write` rewrites every cite with one match. `spec mv` rewrites plain paths to the moved file itself.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers(i) || skipped(d, "L2") {
                continue;
            }
            for c in &d.path_cites {
                let cites =
                    |tail: &str| Finding::error("L2", &c.path, c.line, format!("cites the path {}{tail}", c.item));
                out.push(match ctx.graph.resolve(&c.item) {
                    Resolution::Doc { idx, .. } => {
                        cites("").fix_replace(c.item.clone(), format!("[[{}]]", ctx.corpus.docs[idx].id))
                    }
                    _ => {
                        let f = cites(", which is not a document (no such file)");
                        match by_file_name(ctx, &c.item).as_slice() {
                            [idx] => f.fix_replace(c.item.clone(), format!("[[{}]]", ctx.corpus.docs[*idx].id)),
                            [] => match by_near_path(ctx, &c.item).as_slice() {
                                [] => f,
                                near => f.with_candidates(ids(ctx, near)),
                            },
                            many => f.with_candidates(ids(ctx, many)),
                        }
                    }
                });
            }
        }
    }
}

/// Documents a stale path still names by its file: the same file name, or the same slug once a
/// stamp prefix and a `.<type>` suffix are stripped. Moves and imports keep slugs, so this finds
/// most documents whose old paths were cited.
fn by_file_name(ctx: &Ctx, cited: &str) -> Vec<usize> {
    let file = cited.rsplit('/').next().unwrap_or(cited);
    let docs = &ctx.corpus.docs;
    let same: Vec<usize> = (0..docs.len()).filter(|&i| docs[i].path.rsplit('/').next() == Some(file)).collect();
    if !same.is_empty() {
        return same;
    }
    let mut stem = file.strip_suffix(".md").unwrap_or(file);
    if let Some((s, ty)) = stem.rsplit_once('.')
        && ctx.corpus.cfg.registry.get(ty).is_some()
    {
        stem = s;
    }
    if let Some((stamp, rest)) = stem.split_once('-')
        && crate::model::stamp::is_stamp(stamp)
    {
        stem = rest;
    }
    if stem.is_empty() {
        return vec![];
    }
    (0..docs.len()).filter(|&i| docs[i].slug == stem).collect()
}

/// Documents whose path is a near miss of a cited path (a typo): the closest within a few edits,
/// all of them when tied.
fn by_near_path(ctx: &Ctx, cited: &str) -> Vec<usize> {
    let cited = cited.trim_start_matches("./");
    let docs = &ctx.corpus.docs;
    let scored: Vec<(usize, usize)> = (0..docs.len())
        .map(|i| (distance(cited, &docs[i].path), i))
        .filter(|(d, i)| *d <= 3.max(docs[*i].path.rsplit('/').next().unwrap_or("").len() / 6))
        .collect();
    let Some(best) = scored.iter().map(|(d, _)| *d).min() else { return vec![] };
    scored.into_iter().filter(|(d, _)| *d == best).map(|(_, i)| i).collect()
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
            explain: "rel.supersedes names documents of the same type, rel.implements names rules or decisions (sot, adr, page, design), rel.evidence names evidence, research or audits, rel.depends_on and rel.translation_of name anything, and rel.governs names code paths. Other relation names are reported.",
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
