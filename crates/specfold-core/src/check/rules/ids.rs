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
            explain: "Two documents with the same id make every reference to it ambiguous. Ids come from timestamps or slugs; `spec new` picks the next free second, and `spec mv --slug` renames a slug document. For a timestamp id held by two files, keep one and give the other a new key: `spec new <type> --title <its title> --write`, move its body over with `spec edit <new-id> --text ... --write`, then delete the duplicate file.",
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
            explain: "Aliases carry identifiers from before an import. An alias equal to a current id would hide that document; an alias shared by two documents makes references to it ambiguous, which `spec check` reports at each use with both candidates. `spec fix --rule I2 --pick <id> --write <path>:1` keeps the alias on that document and removes it from the others (frontmatter and alias table).",
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
                        .with_candidates(ids.clone())
                        .fix_cmd(format!(
                            "spec fix --rule I2 --pick {} --write {}:1",
                            crate::shell::word(&ctx.corpus.docs[i].id),
                            super::support::quote(&ctx.corpus.docs[i].path)
                        )),
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
            explain: "The alias table maps identifiers from before an import to current ids. It is written once by `spec import` and only shrinks after that; new documents never need an alias. A range whose base has no table is the import itself and passes; when a later import adds rows, give that commit the trailer `Specfold-Import: yes` (or set SPECFOLD_IMPORT=1 for the run).",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        let Some(file) = &ctx.corpus.aliases.file else { return };
        if matches!(ctx.scope.range, Range::All)
            || !ctx.scope.paths.contains(file)
            || std::env::var_os("SPECFOLD_IMPORT").is_some()
            || import_trailer(ctx)
        {
            return;
        }
        let rows = |t: &str| t.lines().filter(|l| !l.starts_with('#') && l.contains('\t')).count();
        let before = ctx.previous(file).map(|t| rows(&t)).unwrap_or(0);
        let now = ctx.current(file).map(|t| rows(&t)).unwrap_or(0);
        if before > 0 && now > before {
            out.push(Finding::error("I3", file, 1, format!("alias table grew from {before} to {now} rows")));
        }
    }
}

/// Whether a commit in a `--changed <base>` range carries the trailer `Specfold-Import: yes`,
/// which marks it as an import the alias table may grow in. Staged runs have no message yet.
fn import_trailer(ctx: &Ctx) -> bool {
    let range = match &ctx.scope.range {
        Range::Changed(base) => format!("{base}..HEAD"),
        Range::Commit(sha) => crate::workspace::vcs::commit_log(&ctx.corpus.cfg.root, sha),
        _ => return false,
    };
    let Some(commits) = crate::workspace::vcs::commits(&ctx.corpus.cfg.root, &range) else {
        return false;
    };
    commits.iter().any(|c| has_import_trailer(&c.message))
}

/// A `Specfold-Import: yes` line (key case-insensitive, value `yes` or `true`) in a message.
pub fn has_import_trailer(message: &str) -> bool {
    message.lines().any(|l| {
        l.split_once(':').is_some_and(|(k, v)| {
            k.trim().eq_ignore_ascii_case("specfold-import")
                && matches!(v.trim().to_ascii_lowercase().as_str(), "yes" | "true")
        })
    })
}

#[cfg(test)]
mod trailer_tests {
    use super::has_import_trailer;

    #[test]
    fn import_trailer_is_recognised() {
        assert!(has_import_trailer("import docs\n\nSpecfold-Import: yes\n"));
        assert!(has_import_trailer("x\n\nspecfold-import: True"));
        assert!(!has_import_trailer("x\n\nSpecfold-Import: no"));
        assert!(!has_import_trailer("mentions Specfold-Import in prose"));
    }
}
