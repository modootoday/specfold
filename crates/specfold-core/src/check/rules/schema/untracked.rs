//! S9: a full `spec check` reads the files git tracks; documents not yet added are named once,
//! in one warning, instead of being checked as if they were committed.

use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};

pub struct UntrackedLeftOut;

/// How many untracked paths the warning names before it says "+N more".
const SHOWN: usize = 5;
/// How many the JSON `candidates` list carries, so the output stays bounded.
const LISTED: usize = 100;

impl Rule for UntrackedLeftOut {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "S9",
            title: "untracked documents are left out of a full check",
            explain: "A full `spec check` reads only the files git tracks, the local directory ([local] path) and the external roots spec.toml names; `.git` is never read. Markdown files in a spec directory that git does not track yet (a draft not added, another session's leftovers in a shared checkout) are not checked, and this rule reports them in one warning with their count and the first few paths. With `--json` the finding's `candidates` carry the first 100 paths. `git add` the documents to check them, or run `spec check --include-untracked` to read every file on disk as before 0.8.0. Staged and ranged runs are unchanged: `--staged` reads the index and `--changed` includes untracked files.",
            on_commit: false,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        let list = &ctx.corpus.untracked;
        let Some(first) = list.first() else { return };
        let mut named = list.iter().take(SHOWN).cloned().collect::<Vec<_>>().join(", ");
        if list.len() > SHOWN {
            named.push_str(&format!(" (+{} more)", list.len() - SHOWN));
        }
        out.push(
            Finding::warn(
                "S9",
                first,
                1,
                format!(
                    "{} untracked document(s) left out of this check: {named}; git add them, or spec check --include-untracked",
                    list.len()
                ),
            )
            .fix_cmd("spec check --include-untracked")
            .with_candidates(list.iter().take(LISTED).cloned().collect()),
        );
    }
}
