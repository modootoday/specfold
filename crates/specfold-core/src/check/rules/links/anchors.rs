//! L4: anchors are unique within a document.

use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};

pub struct AnchorsUnique;

impl Rule for AnchorsUnique {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "L4",
            title: "anchors are unique within a document",
            explain: "Headings carry stable anchors as `## Title {#ANCHOR}`; references point at them with [[<id>#ANCHOR]]. One anchor per name per document, across its chapters.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers(i) {
                continue;
            }
            let mut seen = std::collections::HashSet::new();
            for a in &d.anchors {
                if !seen.insert(&a.item) {
                    out.push(Finding::error("L4", &a.path, a.line, format!("anchor #{} is defined twice", a.item)));
                }
            }
        }
    }
}
