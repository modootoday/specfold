//! The rule interface. Each rule carries its own explanation, which is the rule's documentation.

use super::context::Ctx;
use super::finding::Finding;

pub struct RuleInfo {
    pub id: &'static str,
    pub title: &'static str,
    /// Why the rule exists and how to fix a finding.
    pub explain: &'static str,
    /// Runs in the pre-commit (staged) scope.
    pub on_commit: bool,
}

pub trait Rule: Sync {
    fn info(&self) -> &RuleInfo;
    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>);
}

/// Rules a document opts out of with `check_skip: [L5, …]`, each needing a reason in the body.
pub fn skipped(doc: &crate::model::Doc, rule: &str) -> bool {
    doc.list_field("check_skip").iter().any(|r| r == rule)
}
