//! A new asset written from a template, at the place its kind is declared.

use super::load::base;
use crate::config::Config;
use crate::config::types::AssetKind;
use crate::workspace::owners::matcher;

/// A new asset from a template: where it goes and what it holds. The first declared directory of
/// the kind is used, else the usual location; the flag says whether a declared root covers it.
pub fn template(
    cfg: &Config,
    kind: AssetKind,
    name: &str,
    description: &str,
    title: &str,
) -> Option<(String, String, bool)> {
    let declared = cfg
        .raw
        .assets
        .iter()
        .find(|r| r.kind == kind && base(r.path.trim_end_matches('/')) == r.path.trim_end_matches('/'));
    let dir = declared.map(|r| r.path.trim_end_matches('/').to_string());
    let one_line = description.replace('\n', " ");
    let (path, text) = match kind {
        AssetKind::Skill => (
            format!("{}/{name}/SKILL.md", dir.as_deref().unwrap_or(".agents/skills")),
            format!(
                "---\nname: {name}\ndescription: {one_line}\n---\n\n# {title}\n\nWhen to use this skill and the steps to follow.\n"
            ),
        ),
        AssetKind::Rule => (
            format!("{}/{name}.mdc", dir.as_deref().unwrap_or(".cursor/rules")),
            format!(
                "---\ndescription: {one_line}\nglobs:\nalwaysApply: false\n---\n\n# {title}\n\nThe rule, with a short example.\n"
            ),
        ),
        AssetKind::Agent => (
            format!("{}/{name}.md", dir.as_deref().unwrap_or(".claude/agents")),
            format!(
                "---\nname: {name}\ndescription: {one_line}\n---\n\nYou are the {title} agent. What you do, and what you hand back.\n"
            ),
        ),
        AssetKind::Instructions => return None,
    };
    let covered = declared.is_some()
        || cfg.raw.assets.iter().any(|r| r.kind == kind && matcher(&r.path).is_some_and(|m| m.is_match(&path)));
    Some((path, text, covered))
}
