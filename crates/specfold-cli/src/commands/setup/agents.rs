//! `spec agents`: the short block to put in an agent's instruction file.

use crate::cli::args::{global::Global, setup::AgentsArgs};
use crate::output::Output;
use anyhow::{Result, bail};

const LINES: &[&str] = &[
    "Design, planning and maintenance documents (sot, adr, plan, research, evidence, audit, page, design, guide, runbook and their files) are managed only with the spec CLI.",
    "Before changing code, run `spec context <path>`; it names the governing documents and their verify commands.",
    "Create documents with `spec new <type> --governs <path> --title ...` and change status or relations with `spec set`; never write a spec file name by hand.",
    "Before committing, run `spec check --staged`; apply mechanical fixes with `spec fix --staged --write`. Rules: `spec explain <rule>`; commands: `spec help --json`.",
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
