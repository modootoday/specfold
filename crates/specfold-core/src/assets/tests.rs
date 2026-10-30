//! Unit tests for `assets`.

use super::*;

#[test]
fn stems_and_bases() {
    assert_eq!(stem(AssetKind::Skill, ".agents/skills/pdf-tools/SKILL.md"), "pdf-tools");
    assert_eq!(stem(AssetKind::Rule, ".cursor/rules/style.mdc"), "style");
    assert_eq!(base(".claude/agents/*.md"), ".claude/agents");
    assert_eq!(base("AGENTS.md"), "AGENTS.md");
    assert!(!shaped(AssetKind::Skill, ".agents/skills/x/notes.md"));
    assert_eq!(join(".agents/skills/x", "../../../docs/a.md#h"), "docs/a.md#h");
    assert_eq!(join("", "../a.md"), "../a.md");
}
