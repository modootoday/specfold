//! Agent assets: skills, rules, subagents and instruction files read beside the documents.

use serde::{Deserialize, Serialize};

/// What an asset root holds. Assets carry no ids; only `spec sync assets` writes them, into roots with `copy_of`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AssetKind {
    /// A directory holding SKILL.md.
    Skill,
    /// A rule file (`.md` or Cursor `.mdc`).
    Rule,
    /// A subagent definition.
    Agent,
    /// AGENTS.md, CLAUDE.md and other instruction files.
    Instructions,
}

impl AssetKind {
    pub fn as_str(self) -> &'static str {
        match self {
            AssetKind::Skill => "skill",
            AssetKind::Rule => "rule",
            AssetKind::Agent => "agent",
            AssetKind::Instructions => "instructions",
        }
    }
}

/// `[[assets]] kind = "skill", path = ".agents/skills"`: a directory or glob of one kind.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssetRoot {
    pub kind: AssetKind,
    /// A directory (everything below it) or a glob, relative to the repository root.
    pub path: String,
    #[serde(default)]
    pub exclude: Vec<String>,
    /// A source directory this root copies (`.agents/skills`); `spec sync assets` writes it here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy_of: Option<String>,
}
