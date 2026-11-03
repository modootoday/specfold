//! S2 and S3: a document's id matches its path, and it sits where its type says.

use crate::check::context::{Ctx, key_line};
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::model::Tier;

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
            explain: "A spec directory holds only type directories, and in them only `<key>-<slug>.<type>.md` files or document bundles. A type with its own directory (`[types.<name>] dir`) holds the same files there, and its documents still in the spec directory are reported with the move that takes them over (`spec mv <id> --write`). A spec.toml that drops or moves a `dir` while documents remain there is refused the same way until each is moved (the old directory is known from spec.toml at HEAD). Files matching `[types.<name>] ignore` (globs relative to the type's directory, such as README.md) are left alone. Anything else (old layouts, notes without a type, loose attachments) is reported. Create documents with `spec new`, attach files with `spec attach`, bring old layouts in with `spec import`.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for s in &ctx.corpus.strays {
            if ctx.scope.covers_path(&s.path) {
                out.push(Finding::error("S3", &s.path, 1, s.reason.clone()));
            }
        }
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if d.tier != Tier::Committed {
                continue;
            }
            let root = ctx.corpus.spec_root_of(i);
            // Whatever the range: the change at fault is the working spec.toml, not these files.
            if let Some(r) = root.filter(|r| r.former) {
                out.push(
                    Finding::error(
                        "S3",
                        &d.path,
                        1,
                        format!("documents remain in former type dir {}; move them with spec mv first", r.dir),
                    )
                    .fix_cmd(format!("spec mv {} --write", crate::suggest::shell::word(&d.id))),
                );
                continue;
            }
            if !ctx.scope.covers(i) {
                continue;
            }
            let Some(dir) = ctx.corpus.cfg.registry.get(&d.id.ty).and_then(|t| t.dir.as_deref()) else { continue };
            if root.is_some_and(|r| r.ty.is_none()) {
                out.push(
                    Finding::error(
                        "S3",
                        &d.path,
                        1,
                        format!(
                            "{} documents live in {dir}/ ([types.{}] dir), not in the spec directory",
                            d.id.ty, d.id.ty
                        ),
                    )
                    .fix_cmd(format!("spec mv {} --write", crate::suggest::shell::word(&d.id))),
                );
            }
        }
    }
}
