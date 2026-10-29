//! A1 and A2: agent assets declared under `[[assets]]` load in the tools that read them. Only a
//! missing name or description is an error, because a tool then refuses or silently skips the
//! asset; everything else the specifications ask for is a warning.

use crate::assets::{self, Asset};
use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::config::types::AssetKind;
use std::collections::BTreeMap;

pub struct AssetFrontmatter;
pub struct AssetNamesUnique;

const SKILL_KEYS: &[&str] = &["name", "description", "license", "compatibility", "metadata", "allowed-tools"];
const AGENT_KEYS: &[&str] = &["name", "description", "tools", "model", "color"];
const MDC_KEYS: &[&str] = &["description", "globs", "alwaysApply"];
const MAX_NAME: usize = 64;
const MAX_DESCRIPTION: usize = 1024;
const MAX_COMPATIBILITY: usize = 500;
const MAX_SKILL_LINES: usize = 500;

fn line_of(a: &Asset, key: &str) -> usize {
    let prefix = format!("{key}:");
    a.text.lines().position(|l| l.starts_with(&prefix)).map(|i| i + 1).unwrap_or(1)
}

/// Why a name breaks the Agent Skills rule: 1–64 lowercase letters, digits and hyphens, no
/// leading, trailing or doubled hyphen.
fn name_problem(name: &str) -> Option<String> {
    if name.chars().count() > MAX_NAME {
        return Some(format!("is longer than {MAX_NAME} characters"));
    }
    if !name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        return Some("may use only lowercase letters, digits and hyphens".into());
    }
    if name.starts_with('-') || name.ends_with('-') || name.contains("--") {
        return Some("may not start or end with a hyphen or contain '--'".into());
    }
    None
}

fn text_field<'a>(a: &'a Asset, key: &str) -> Option<&'a str> {
    a.field(key).map(str::trim).filter(|s| !s.is_empty())
}

fn required(a: &Asset, out: &mut Vec<Finding>) {
    for key in ["name", "description"] {
        if text_field(a, key).is_none() {
            let what = if a.kind == AssetKind::Skill { "skill" } else { "subagent" };
            out.push(Finding::error(
                "A1",
                &a.path,
                line_of(a, key),
                format!("{what} has no {key}; tools skip or refuse it without one"),
            ));
        }
    }
}

fn unknown_keys(a: &Asset, known: &[&str], out: &mut Vec<Finding>) {
    let Some(fields) = &a.fields else { return };
    for k in fields.keys() {
        if known.contains(&k.as_str()) {
            continue;
        }
        let msg = match known.iter().find(|n| n.eq_ignore_ascii_case(k)) {
            Some(n) => format!("key '{k}' is not recognized; the field is spelled '{n}'"),
            None => format!("key '{k}' is not one {} reads ({})", a.kind.as_str(), known.join(", ")),
        };
        out.push(Finding::warn("A1", &a.path, line_of(a, k), msg));
    }
}

fn skill(a: &Asset, out: &mut Vec<Finding>) {
    required(a, out);
    unknown_keys(a, SKILL_KEYS, out);
    if let Some(name) = text_field(a, "name") {
        if let Some(why) = name_problem(name) {
            out.push(Finding::warn("A1", &a.path, line_of(a, "name"), format!("skill name '{name}' {why}")));
        }
        if name != a.stem {
            out.push(Finding::warn(
                "A1",
                &a.path,
                line_of(a, "name"),
                format!("skill name '{name}' differs from its directory '{}'; they must match", a.stem),
            ));
        }
    }
    if let Some(d) = text_field(a, "description")
        && d.chars().count() > MAX_DESCRIPTION
    {
        out.push(Finding::warn(
            "A1",
            &a.path,
            line_of(a, "description"),
            format!("description is longer than {MAX_DESCRIPTION} characters"),
        ));
    }
    if let Some(c) = a.field("compatibility")
        && c.chars().count() > MAX_COMPATIBILITY
    {
        out.push(Finding::warn(
            "A1",
            &a.path,
            line_of(a, "compatibility"),
            format!("compatibility is longer than {MAX_COMPATIBILITY} characters"),
        ));
    }
    let lines = a.text.lines().count();
    if lines > MAX_SKILL_LINES {
        out.push(Finding::warn(
            "A1",
            &a.path,
            1,
            format!(
                "SKILL.md has {lines} lines; keep it under {MAX_SKILL_LINES} and move detail into referenced files"
            ),
        ));
    }
}

fn agent(a: &Asset, out: &mut Vec<Finding>) {
    required(a, out);
    unknown_keys(a, AGENT_KEYS, out);
    if let Some(name) = text_field(a, "name")
        && let Some(why) = name_problem(name)
    {
        out.push(Finding::warn("A1", &a.path, line_of(a, "name"), format!("subagent name '{name}' {why}")));
    }
}

fn rule(a: &Asset, out: &mut Vec<Finding>) {
    let cursor = a.path.starts_with(".cursor/rules/") || a.path.contains("/.cursor/rules/");
    if cursor && !a.path.ends_with(".mdc") {
        out.push(Finding::warn("A1", &a.path, 1, "Cursor reads only .mdc files in .cursor/rules; rename it to .mdc"));
        return;
    }
    if !a.path.ends_with(".mdc") {
        return;
    }
    unknown_keys(a, MDC_KEYS, out);
    let always = a.fields.as_ref().and_then(|f| f.get("alwaysApply")).and_then(|v| v.as_bool()).unwrap_or(false);
    let globs = a.fields.as_ref().and_then(|f| f.get("globs")).is_some_and(|v| !v.is_null() && v != "");
    if !always && !globs && text_field(a, "description").is_none() {
        out.push(Finding::warn(
            "A1",
            &a.path,
            1,
            "rule has no description, globs or alwaysApply, so it is only applied when named by hand",
        ));
    }
}

impl Rule for AssetFrontmatter {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "A1",
            title: "agent assets have the frontmatter their tools read",
            explain: "Assets declared under [[assets]] in spec.toml are checked against the format of the tool that loads them. A skill (SKILL.md, agentskills.io) and a subagent need a non-empty name and description: without them a tool skips or refuses the file, so that is an error. The rest are warnings: a skill name is 1-64 lowercase letters, digits and hyphens without a leading, trailing or doubled hyphen and equals its directory; description stays under 1024 characters and compatibility under 500; SKILL.md stays under 500 lines; keys a tool does not read, or recognized keys in the wrong case, are named. Cursor reads only .mdc files in .cursor/rules, and a rule with no description, globs or alwaysApply is never applied on its own. Fix the frontmatter in the asset; specfold never writes assets.",
            on_commit: false,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for a in assets::load(&ctx.corpus.cfg) {
            if let Some(why) = &a.invalid {
                out.push(Finding::error("A1", &a.path, 1, format!("frontmatter cannot be read: {why}")));
                continue;
            }
            match a.kind {
                AssetKind::Skill => skill(&a, out),
                AssetKind::Agent => agent(&a, out),
                AssetKind::Rule => rule(&a, out),
                AssetKind::Instructions => {}
            }
        }
    }
}

/// The tool directory an asset sits under (`.claude`, `.agents`, `.cursor`, or the repository).
fn tool_dir(path: &str) -> &str {
    path.split('/').find(|c| c.starts_with('.')).unwrap_or("")
}

impl Rule for AssetNamesUnique {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "A2",
            title: "asset names are unique within a tool",
            explain: "A tool loads one asset per name: two skills or two subagents with the same name under one tool directory (.claude, .agents, .cursor …) means one of them silently never loads, chosen by read order. Rename one or remove it. The same name under different tool directories is a copy for another tool, not a collision; whether such copies agree is an audit question.",
            on_commit: false,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        let mut seen: BTreeMap<(AssetKind, String, String), Vec<String>> = BTreeMap::new();
        for a in assets::load(&ctx.corpus.cfg) {
            if matches!(a.kind, AssetKind::Instructions) {
                continue;
            }
            let key = (a.kind, tool_dir(&a.path).to_string(), a.name().to_string());
            seen.entry(key).or_default().push(a.path.clone());
        }
        for ((kind, _, name), paths) in seen {
            if paths.len() < 2 {
                continue;
            }
            for p in &paths {
                let others: Vec<&str> = paths.iter().filter(|o| *o != p).map(String::as_str).collect();
                out.push(
                    Finding::warn(
                        "A2",
                        p,
                        1,
                        format!(
                            "{} name '{name}' is also used by {}; only one loads",
                            kind.as_str(),
                            others.join(", ")
                        ),
                    )
                    .with_candidates(others.iter().map(|s| s.to_string()).collect()),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert!(name_problem("pdf-tools").is_none());
        assert!(name_problem("PDF").is_some());
        assert!(name_problem("-x").is_some());
        assert!(name_problem("a--b").is_some());
        assert!(name_problem(&"a".repeat(65)).is_some());
        assert_eq!(tool_dir(".claude/skills/x/SKILL.md"), ".claude");
        assert_eq!(tool_dir("docs/AGENTS.md"), "");
    }
}
