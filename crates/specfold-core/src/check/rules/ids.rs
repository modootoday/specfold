//! I rules: ids and aliases name one document each, and the alias table stays frozen.

use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::check::scope::Range;
use std::collections::BTreeMap;

pub struct UniqueIds;
pub struct AliasesClean;
pub struct AliasTableFrozen;

impl Rule for UniqueIds {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "I1",
            title: "ids are unique across every root",
            explain: "Two documents with the same id make every reference to it ambiguous. Ids come from timestamps or slugs; `spec new` picks the next free second, and `spec mv --slug` renames a slug document.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        let mut seen: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            seen.entry(d.id.to_string()).or_default().push(i);
        }
        for (id, list) in seen {
            if list.len() < 2 || !list.iter().any(|&i| ctx.scope.covers(i)) {
                continue;
            }
            let paths: Vec<String> = list.iter().map(|&i| ctx.corpus.docs[i].path.clone()).collect();
            for p in &paths {
                out.push(
                    Finding::error("I1", p, 1, format!("id {id} is used by {} documents", paths.len()))
                        .with_candidates(paths.clone()),
                );
            }
        }
    }
}

impl Rule for AliasesClean {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "I2",
            title: "aliases do not shadow ids and point at one document",
            explain: "Aliases carry identifiers from before an import. An alias equal to a current id would hide that document; an alias shared by two documents makes references to it ambiguous, which `spec check` reports at each use with both candidates.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for (alias, list) in &ctx.graph.by_alias {
            if ctx.graph.by_id.contains_key(alias) {
                for &i in list {
                    out.push(Finding::error(
                        "I2",
                        &ctx.corpus.docs[i].path,
                        1,
                        format!("alias '{alias}' is also a document id"),
                    ));
                }
            }
            if list.len() > 1 && list.iter().any(|&i| ctx.scope.covers(i)) {
                let ids: Vec<String> = list.iter().map(|&i| ctx.corpus.docs[i].id.to_string()).collect();
                for &i in list {
                    out.push(
                        Finding::warn(
                            "I2",
                            &ctx.corpus.docs[i].path,
                            1,
                            format!("alias '{alias}' names {} documents", ids.len()),
                        )
                        .with_candidates(ids.clone()),
                    );
                }
            }
        }
    }
}

impl Rule for AliasTableFrozen {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "I3",
            title: "the alias table gains no rows",
            explain: "The alias table maps identifiers from before an import to current ids. It is written once by `spec import` and only shrinks after that; new documents never need an alias. Set SPECFOLD_IMPORT=1 for the commit that writes it.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        let Some(file) = &ctx.corpus.aliases.file else { return };
        if matches!(ctx.scope.range, Range::All)
            || !ctx.scope.paths.contains(file)
            || std::env::var_os("SPECFOLD_IMPORT").is_some()
        {
            return;
        }
        let rows = |t: &str| t.lines().filter(|l| !l.starts_with('#') && l.contains('\t')).count();
        let before = ctx.previous(file).map(|t| rows(&t)).unwrap_or(0);
        let now = ctx.current(file).map(|t| rows(&t)).unwrap_or(0);
        if now > before {
            out.push(Finding::error("I3", file, 1, format!("alias table grew from {before} to {now} rows")));
        }
    }
}
