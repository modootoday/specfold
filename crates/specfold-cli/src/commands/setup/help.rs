//! `spec help`: the command tree; with --json, everything an agent needs to call every command.

use crate::cli::args::{global::Global, setup::HelpArgs};
use crate::cli::describe;
use crate::output::Output;
use anyhow::{Result, bail};
use serde_json::to_value;

pub fn run(_g: &Global, a: &HelpArgs) -> Result<Output> {
    let all = describe::commands();
    let picked: Vec<_> = match &a.command {
        Some(name) => {
            let c: Vec<_> = all.iter().filter(|c| &c.name == name).cloned().collect();
            if c.is_empty() {
                bail!("no command '{name}'");
            }
            c
        }
        None => all,
    };
    if a.markdown {
        return Ok(markdown(&picked));
    }
    let mut o = Output::new("help")
        .unpaged()
        .field("global", to_value(describe::global_args())?)
        .field("exit_codes", serde_json::json!({"0": "nothing to report", "1": "findings or an ambiguous answer", "2": "usage error", "3": "environment error"}));
    for c in &picked {
        let mut text = format!("{:<9} {}", c.name, c.about);
        if a.command.is_some() {
            for arg in &c.args {
                let flag = arg.long.as_ref().map(|l| format!("--{l}")).unwrap_or_else(|| format!("<{}>", arg.name));
                text.push_str(&format!("\n  {flag:<18} {}{}", arg.help, if arg.required { " (required)" } else { "" }));
            }
            for e in &c.examples {
                text.push_str(&format!("\n  e.g. {e}"));
            }
        }
        o = o.item(to_value(c)?, text);
    }
    Ok(o.next(if a.command.is_some() { "spec explain" } else { "spec help <command>" }))
}

/// The README command table: one row per command, from the same data as --json.
fn markdown(cmds: &[describe::CommandInfo]) -> Output {
    let mut o = Output::new("help").unpaged().head("| command | what it does |").head("| ------- | ------------ |");
    for c in cmds {
        let about = c.about.trim_end_matches('.').replace('|', "\\|");
        o = o.item(to_value(c).unwrap_or_default(), format!("| `spec {}` | {about} |", c.name));
    }
    o
}
