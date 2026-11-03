//! S2 and S3: a document's id matches its path, and it sits where its type says.

use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::check::rules::support::key_line;

pub struct IdMatchesPath;
pub struct Placement;

impl Rule for IdMatchesPath {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "S2",
            title: "id matches the path",
            explain: "A document's id is computed from where it lives: <type>:<owner>/<key>. The id written in frontmatter must equal it, so a copied or moved file cannot keep a stale id. Move documents with `spec mv`, which rewrites both.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers(i) {
                continue;
            }
            let Some(declared) = d.str_field("id") else { continue };
            let expected = d.id.to_string();
            if declared != expected {
                let line = key_line(ctx, &d.path, "id");
                out.push(
                    Finding::error(
                        "S2",
                        &d.path,
                        line,
                        format!("id is '{declared}' but the path makes it '{expected}'"),
                    )
                    .fix_replace(format!("id: {declared}"), format!("id: {expected}")),
                );
            }
        }
    }
}

impl Rule for Placement {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "S3",
            title: "every file in a spec directory belongs to a document",
            explain: "A spec directory holds only type directories, and in them only `<key>-<slug>.<type>.md` files or document bundles. Anything else (old layouts, notes without a type, loose attachments) is reported. Create documents with `spec new`, attach files with `spec attach`, bring old layouts in with `spec import`.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for s in &ctx.corpus.strays {
            if ctx.scope.covers_path(&s.path) {
                out.push(Finding::error("S3", &s.path, 1, s.reason.clone()));
            }
        }
    }
}
