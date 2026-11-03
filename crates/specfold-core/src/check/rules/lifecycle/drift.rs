//! D1: in a staged or ranged run, code changed under an active document's `governs` while the
//! document, and every change note pointing at it, stayed as they were.

use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::skipped;
use crate::check::rule::{Rule, RuleInfo};
use crate::corpus::graph::query::governing;
use crate::model::Tier;
use std::collections::{BTreeMap, BTreeSet};

pub struct GovernedCodeDrift;

/// How many changed paths a finding names before it says "+N more".
const SHOWN: usize = 5;

impl Rule for GovernedCodeDrift {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "D1",
            title: "changed code moves its governing document",
            explain: "In a staged or ranged run (`spec check --staged`, `--changed`), each changed file that is not a document is matched against the explicit `rel.governs` globs of active, committed documents. A document whose governed code changed is expected to change in the same range, or to be named by a `change` note that changes in the range, or to cover a path that such a note governs (a note on the source file speaks for its test beside it); otherwise this warning lists the changed paths. Documents that govern only through their owner directory are not asked, and a full run never reports D1. Deleted files and the old paths of renamed files count as changed. With `[check] d1 = \"specific\"` only the most specific of the documents governing a file are asked, so an overview governing a whole tree (`governs: [services]`) is not asked about a file a narrower document governs. Update the document (or its `verified_at`), link a change note already in the range (`spec set <note> rel.depends_on+=<id> --write`, the fix D1 prints when there is one), or write a change note that links it: `spec new change --title … --depends-on <id> --write` (a `[[<id>]]` link in its body works too); where the change type is disabled, the fix pins the document instead: `spec set <id> verified_at=HEAD --write`. See it with `spec check --staged --rule D1`.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        if ctx.scope.is_all() {
            return;
        }
        for g in &ctx.scope.submodules_missing {
            out.push(Finding::warn(
                "D1",
                g,
                1,
                format!(
                    "{g}: submodule objects missing; the whole directory counts as changed; next: git submodule update"
                ),
            ));
        }
        let c = ctx.corpus;
        let own: BTreeSet<&String> =
            c.docs.iter().flat_map(|d| std::iter::once(&d.path).chain(&d.chapters).chain(&d.bundle_files)).collect();
        let mut drift: BTreeMap<usize, Vec<&str>> = BTreeMap::new();
        let gone: Vec<String> = ctx.scope.gone.iter().map(|p| format!("{p} (removed)")).collect();
        let changed = ctx.scope.paths.iter().map(|p| (p.as_str(), p.as_str()));
        let removed = ctx.scope.gone.iter().zip(&gone).map(|(p, shown)| (p.as_str(), shown.as_str()));
        let specific = c.cfg.raw.check.d1_specific();
        for (p, shown) in changed.chain(removed).filter(|(p, _)| !own.contains(&p.to_string())) {
            let asked: Vec<_> = governing(c, p)
                .into_iter()
                .filter(|g| {
                    let d = &c.docs[g.idx];
                    let explicit = d.rel().get("governs").is_some_and(|v| !v.is_empty());
                    explicit
                        && d.tier == Tier::Committed
                        && d.status().as_deref() == Some("active")
                        && !skipped(d, "D1")
                })
                .collect();
            // `governing` lists the narrowest claims first.
            let top = asked.first().map(|g| g.specificity).unwrap_or(0);
            for g in asked.into_iter().filter(|g| !specific || g.specificity == top) {
                drift.entry(g.idx).or_default().push(shown);
            }
        }
        let notes: Vec<usize> = ctx.scope.touched.iter().copied().filter(|&i| c.docs[i].id.ty == "change").collect();
        for (idx, paths) in drift {
            let moved = ctx.scope.touched.contains(&idx)
                || ctx.graph.edges_to(idx).any(|e| notes.contains(&e.from))
                || notes.iter().any(|&n| governs_inside(c, n, idx));
            if moved {
                continue;
            }
            let d = &c.docs[idx];
            let mut named = paths.iter().take(SHOWN).copied().collect::<Vec<_>>().join(", ");
            if paths.len() > SHOWN {
                named.push_str(&format!(" (+{} more)", paths.len() - SHOWN));
            }
            // A note already in the change is linked rather than joined by a second one.
            let note = notes.iter().find(|&&n| c.docs[n].id.owner == d.id.owner).or(notes.first());
            let fix = match note {
                Some(&n) => format!(
                    "spec set {} rel.depends_on+={} --write",
                    crate::suggest::shell::word(&c.docs[n].id),
                    crate::suggest::shell::word(&d.id)
                ),
                None if c.cfg.registry.get("change").is_some() => format!(
                    "spec new change --title {} --depends-on {} --write",
                    crate::suggest::shell::quote(&format!("Code change under {}", d.id.owner)),
                    crate::suggest::shell::word(&d.id)
                ),
                // Without change notes the document itself moves: a pin is the smallest change.
                None => format!("spec set {} verified_at=HEAD --write", crate::suggest::shell::word(&d.id)),
            };
            out.push(
                Finding::warn(
                    "D1",
                    &d.path,
                    1,
                    format!("{named} changed under {}'s governs, but the document and its change notes did not", d.id),
                )
                .fix_cmd(fix),
            );
        }
    }
}

/// D1: whether change note `note` governs a path inside what document `doc` governs, so the note
/// speaks for changes there (a test file beside the source the note names). A note governing a
/// wider tree does not count: it says nothing about this document.
fn governs_inside(c: &crate::corpus::Corpus, note: usize, doc: usize) -> bool {
    let theirs = c.docs[note].rel().remove("governs").unwrap_or_default();
    let mine = c.docs[doc].rel().remove("governs").unwrap_or_default();
    theirs.iter().map(|g| crate::workspace::placement::literal_base(g)).any(|p| {
        mine.iter().any(|s| {
            let base = crate::workspace::placement::literal_base(s);
            crate::workspace::owners::matcher(s).is_some_and(|m| m.is_match(&p))
                || (!base.is_empty() && crate::paths::is_under(&p, &base))
        })
    })
}
