//! A4: agent assets name paths inside the repository, not on one person's machine. A home
//! directory path works for whoever wrote it and for nobody else.

use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};

pub struct NoMachinePaths;

const HOMES: &[&str] = &["/Users/", "/home/", ":\\Users\\", ":/Users/"];

/// The machine-specific paths on a line: a home directory at the start of a word, followed by
/// a user name. A URL path (`example.com/home/`) is not one.
fn found(line: &str) -> Vec<String> {
    let mut out = vec![];
    for home in HOMES {
        let drive = home.starts_with(':');
        for (at, _) in line.match_indices(home) {
            let start = if drive { at.checked_sub(1) } else { Some(at) };
            let Some(start) = start else { continue };
            if drive && !line[start..].starts_with(|c: char| c.is_ascii_alphabetic()) {
                continue;
            }
            let before = line[..start].chars().next_back();
            if before.is_some_and(|c| !(c.is_whitespace() || "\"'`([<=,:".contains(c))) {
                continue;
            }
            let rest = &line[at + home.len()..];
            let user: String = rest.chars().take_while(|c| c.is_alphanumeric() || "._-".contains(*c)).collect();
            // The hosted CI runner's home is the same on every run, not one person's machine.
            if user.is_empty() || (*home == "/home/" && user == "runner") {
                continue;
            }
            out.push(line[start..at + home.len() + user.len()].to_string());
        }
    }
    out
}

impl Rule for NoMachinePaths {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "A4",
            title: "agent assets use repository paths, not machine paths",
            explain: "A skill, rule or instruction file (declared in spec.toml, or at a usual place such as AGENTS.md, CLAUDE.md or .agents/skills/*/SKILL.md) is read on every contributor's machine and in CI. A path under one person's home directory (/Users/<name>, /home/<name>, C:\\Users\\<name>) exists only there; the hosted CI runner's /home/runner/ is the same everywhere and is not reported, so the agent following it fails or reads the wrong file elsewhere. Write the path relative to the repository root instead. Reported as warnings; there is no automatic fix because only the author knows where the file lives in the repository.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for (path, text) in super::hidden::agent_files(ctx) {
            for (i, line) in text.lines().enumerate() {
                for p in found(line) {
                    let msg = format!("machine-specific path {p}: write it relative to the repository root");
                    out.push(Finding::warn("A4", &path, i + 1, msg));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::found;

    #[test]
    fn home_paths_but_not_url_paths() {
        assert_eq!(found("run `/Users/ana/bin/x` then"), vec!["/Users/ana"]);
        assert_eq!(found("see /home/dev-1/repo"), vec!["/home/dev-1"]);
        assert_eq!(found(r"open C:\Users\Kim\x"), vec![r"C:\Users\Kim"]);
        assert!(found("https://example.com/home/page").is_empty());
        assert!(found("the /home/ directory").is_empty());
        assert!(found("cache in /home/runner/.cargo").is_empty());
        assert_eq!(found("see /home/runner2/x"), vec!["/home/runner2"]);
        assert!(found("~/projects and ./home/x").is_empty());
    }
}
