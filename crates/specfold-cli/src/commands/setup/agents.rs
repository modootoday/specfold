//! `spec agents`: the short block to put in an agent's instruction file.

use crate::cli::args::{global::Global, setup::AgentsArgs};
use crate::output::Output;
use anyhow::{Result, bail};

const LINES: &[&str] = &[
    "Design, planning and maintenance documents (sot, adr, plan, research, evidence, audit, page, design, guide, runbook, change and their files) are managed only with the spec CLI.",
    "Before changing code, run `spec context <path>`; it names the governing documents and their verify commands.",
    "Before changing a document, run `spec graph impact <id>` to see what reaches it (relation and distance); `spec graph chain <id>` shows what it replaced and what replaced it; `spec graph --format box|mermaid|dot` prints the whole graph as a diagram.",
    "Create documents with `spec new <type> --governs <path> --title ...` and change status or relations with `spec set`; never write a spec file name by hand.",
    "Change a body with `spec edit <id>`: `--section <h>`, `--new-section <h> [--after <h>]`, or one table row with `--row <key> --cell Status=done`; preview, then add `--write`.",
    "Before committing, run `spec check --staged`; apply mechanical fixes with `spec fix --staged --write`. `--summary` counts findings per rule, `-q` lists errors only. Rules: `spec explain <rule>`; commands: `spec help --json`.",
    "Agent skills, rules and subagents are declared under `[[assets]]`: list them with `spec assets`, start one with `spec new skill|rule|agent --title ...`; `.agents/` holds the source; refresh tool copies (roots with `copy_of`) with `spec sync assets --write`.",
    "`spec audit` lists what passes checks but needs attention: stale documents (pin one with `spec set <id> verified_at=HEAD`), finished plans (`--kind finished`), asset copies that drifted (`--kind drift`).",
];

pub fn run(_g: &Global, a: &AgentsArgs) -> Result<Output> {
    let file = match a.target.as_str() {
        "claude" => "CLAUDE.md",
        "codex" | "generic" => "AGENTS.md",
        "gemini" => "GEMINI.md",
        "cursor" => ".cursor/rules/spec.mdc",
        "copilot" => ".github/copilot-instructions.md",
        other => bail!("unknown agent '{other}' (claude, codex, gemini, cursor, copilot, generic)"),
    };
    let mut o = Output::new("agents").unpaged().field("file", file).field("lines", LINES.to_vec());
    o = o.head(format!("# add to {file}"));
    for l in LINES {
        o = o.head(format!("- {l}"));
    }
    Ok(o.next("spec hook print --for git"))
}
