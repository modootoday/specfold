//! L2: documents are cited by id, not by path.

use super::targets::ids;
use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::check::rules::support::{distance, skipped};
use crate::graph::Resolution;

pub struct NoPathCites;

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
