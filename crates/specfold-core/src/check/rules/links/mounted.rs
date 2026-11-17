//! L10: in a repository a workspace mounts, governed paths are written from the workspace root.

use crate::check::context::{Ctx, key_line};
use crate::check::finding::Finding;
use crate::check::rule::skipped;
use crate::check::rule::{Rule, RuleInfo};
use crate::suggest::shell::word;

pub struct GovernsMounted;

impl Rule for GovernsMounted {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "L10",
            title: "governed paths start with the mount",
            explain: "With `[owners] mount = \"<dir>\"`, this repository is mounted at `<dir>` inside a larger workspace, and every `rel.governs` entry is written from that workspace's root, so a document reads the same from both repositories: `<dir>/src/**`, not `src/**`. An entry that is not the mount or inside it is an error here, before the workspace refuses it as governing its own code. The fix rewrites the entry under the mount. Entries in other projects (`<remote>:<path>`) are left alone, as P1 leaves them. L5, P1, D1 and `spec context` read `<dir>/<path>` as `<path>` from this repository's root. Without a mount the rule does nothing.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        let owners = &ctx.corpus.cfg.raw.owners;
        let Some(mount) = owners.mount_dir() else { return };
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers(i) || skipped(d, "L10") {
                continue;
            }
            for g in d.rel().remove("governs").unwrap_or_default() {
                if owners.is_mounted(&g) || crate::workspace::projects::is_external(&ctx.corpus.cfg, &g) {
                    continue;
                }
                let rest = g.trim_start_matches("./").trim_start_matches('/');
                let fixed = if rest.is_empty() || rest == "." { mount.to_string() } else { format!("{mount}/{rest}") };
                out.push(
                    Finding::error(
                        "L10",
                        &d.path,
                        key_line(ctx, &d.path, "rel"),
                        format!("governs '{g}' is written from this repository; the workspace mounts it at {mount}, so write '{fixed}'"),
                    )
                    .fix_cmd(format!(
                        "spec set {} {} {} --write",
                        word(&d.id),
                        word(format!("rel.governs-={g}")),
                        word(format!("rel.governs+={fixed}"))
                    )),
                );
            }
        }
    }
}
