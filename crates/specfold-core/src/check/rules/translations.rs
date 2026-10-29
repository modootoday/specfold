//! R1: a translation records the source body it was made from, and says when the source moved on.

use super::support::key_line;
use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::translation::{self, KEY, RELATION};

pub struct TranslationsCurrent;

impl Rule for TranslationsCurrent {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "R1",
            title: "translations are current with their source",
            explain: "A translated document names its source with rel.translation_of and records the source it was made from as source_rev: sha256:<12 hex>, a hash of the source body with frontmatter removed and whitespace collapsed (no git history needed, and reformatting does not count). When the source body changes, the translation is reported as behind until it is re-translated and source_rev is recorded again with `spec set <id> --source-rev`. A translation without source_rev gets the same command. A source that does not resolve is reported by L3. Reported as warnings; `spec census --translations` lists every translation and the sources each language lacks.",
            on_commit: false,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        let c = ctx.corpus;
        for (i, d) in c.docs.iter().enumerate() {
            let rel = d.rel();
            let Some(targets) = rel.get(RELATION) else { continue };
            let Some(src) = translation::source_of(c, ctx.graph, i) else { continue };
            if !(ctx.scope.covers(i) || ctx.scope.covers(src)) {
                continue;
            }
            let line = key_line(ctx, &d.path, KEY);
            let fix = format!("spec set {} --source-rev", d.id);
            if targets.len() > 1 {
                out.push(Finding::warn(
                    "R1",
                    &d.path,
                    key_line(ctx, &d.path, "rel"),
                    format!("rel.translation_of names {} documents; a translation has one source", targets.len()),
                ));
            }
            let source = &c.docs[src];
            let now = ctx.current(&source.path).map(|t| translation::rev_of_text(&t));
            match d.str_field(KEY) {
                None => out.push(
                    Finding::warn("R1", &d.path, 1, format!("translation of {} records no source_rev", source.id))
                        .fix_cmd(fix),
                ),
                Some(r) if now.as_deref().is_some_and(|n| n != r) => out.push(
                    Finding::warn(
                        "R1",
                        &d.path,
                        line,
                        format!("{} changed since source_rev {r}: update the translation", source.id),
                    )
                    .fix_cmd(fix),
                ),
                Some(_) => {}
            }
        }
    }
}
