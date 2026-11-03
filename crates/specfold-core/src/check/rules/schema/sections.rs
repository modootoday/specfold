//! S7: a document has the sections its type requires (`[types.<t>] sections = [...]`).

use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};

pub struct RequiredSections;

/// The headings of a Markdown text outside code fences, without `#` marks or a `{#anchor}`.
fn headings(text: &str) -> Vec<String> {
    let mut fence = crate::model::markdown::Fence::default();
    let mut out = vec![];
    for l in text.lines() {
        if fence.step(l) || fence.inside() {
            continue;
        }
        let Some((_, h)) = crate::model::markdown::atx(l) else { continue };
        let h = match h.rfind(" {#") {
            Some(i) if h.ends_with('}') => &h[..i],
            _ => h,
        };
        out.push(h.trim().to_lowercase());
    }
    out
}

impl Rule for RequiredSections {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "S7",
            title: "a document has the sections its type requires",
            explain: "A type can require headings in every document through `[types.<type>] sections = [\"Context\", \"Decision\"]` in spec.toml. A heading of any level counts; case and a trailing {#anchor} are ignored. No type requires sections unless configured. `spec edit <id> --new-section <name> --text \"...\" --write` adds one (`--after <heading>` places it).",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers(i) {
                continue;
            }
            let Some(ty) = ctx.corpus.cfg.registry.get(&d.id.ty) else { continue };
            if ty.sections.is_empty() {
                continue;
            }
            let mut have = vec![];
            for p in std::iter::once(&d.path).chain(&d.chapters) {
                if let Some(t) = ctx.current(p) {
                    have.extend(headings(&t));
                }
            }
            for s in &ty.sections {
                if !have.contains(&s.trim().to_lowercase()) {
                    let msg = format!("missing section '{s}' (required for {})", ty.name);
                    let add = format!(
                        "spec edit {} {} --text \"…\" --write",
                        crate::suggest::shell::word(&d.id),
                        crate::suggest::shell::opt("--new-section", s.trim())
                    );
                    out.push(Finding::warn("S7", &d.path, 1, msg).fix_cmd(add));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn headings_skip_fences_and_anchors() {
        let h = super::headings("# A\n## Decision {#d}\n```\n# not\n```\n#nospace\n");
        assert_eq!(h, ["a", "decision"]);
    }
}
