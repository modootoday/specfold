//! The command tree as data, read from the clap definition. `spec help --json` prints it and the
//! MCP server turns it into tools, so neither can drift from the real CLI.

use super::args::Cli;
use clap::{ArgAction, CommandFactory};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ArgInfo {
    pub name: String,
    /// `--long` flag, or None for a positional argument.
    pub long: Option<String>,
    pub help: String,
    pub required: bool,
    /// `bool`, `string` or `string[]`.
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommandInfo {
    pub name: String,
    pub about: String,
    /// Other names that run it.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    pub args: Vec<ArgInfo>,
    pub examples: Vec<String>,
    /// The prose after the examples in `--help`: limits, settings, what a refusal means.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub notes: String,
    /// The schema name of its --json output.
    pub output: String,
    /// CLI-2: what it can do at worst: read, write (files in the repository), destructive
    /// (deletes or overwrites; needs --yes as well) or external (sends or runs something outside).
    pub risk: String,
}

/// CLI-2: the risk level of each command, its worst case with every flag.
pub fn risk(name: &str) -> &'static str {
    match name {
        "new" | "set" | "edit" | "mv" | "fix" | "init" | "import" | "audit" | "check" | "guard" => "write",
        "sync" | "export" => "destructive",
        "attach" | "fetch" | "verify" | "plugins" => "external",
        _ => "read",
    }
}

const GLOBAL: &[&str] = &["json", "jsonl", "limit", "offset", "max_bytes", "dir", "no_local", "help", "version"];

pub fn commands() -> Vec<CommandInfo> {
    let cmd = Cli::command();
    cmd.get_subcommands().map(info).collect()
}

pub fn global_args() -> Vec<ArgInfo> {
    let cmd = Cli::command();
    cmd.get_arguments().filter(|a| a.get_id() != "help" && a.get_id() != "version").map(arg_info).collect()
}

fn info(c: &clap::Command) -> CommandInfo {
    let after = c.get_after_help().map(|h| h.to_string()).unwrap_or_default();
    let examples = after.lines().map(str::trim).filter(|l| l.starts_with("spec ")).map(str::to_string).collect();
    let notes = notes_of(&after);
    let mut args: Vec<ArgInfo> =
        c.get_arguments().filter(|a| !GLOBAL.contains(&a.get_id().as_str())).map(arg_info).collect();
    // The server takes no arguments of its own, but the repository it serves is the global --dir.
    if c.get_name() == "lsp" {
        args.push(ArgInfo {
            name: "dir".into(),
            long: Some("dir".into()),
            help: "The repository to serve (default: the editor's root folder)".into(),
            required: false,
            kind: "string".into(),
            default: None,
        });
    }
    CommandInfo {
        name: c.get_name().to_string(),
        about: c.get_about().map(|s| s.to_string()).unwrap_or_default(),
        aliases: c.get_visible_aliases().map(str::to_string).collect(),
        args,
        examples,
        notes,
        output: format!("spec.{}.v1", c.get_name()),
        risk: risk(c.get_name()).to_string(),
    }
}

fn arg_info(a: &clap::Arg) -> ArgInfo {
    let kind = match a.get_action() {
        ArgAction::SetTrue | ArgAction::SetFalse => "bool",
        ArgAction::Append => "string[]",
        _ if a.get_num_args().is_some_and(|n| n.max_values() > 1) => "string[]",
        _ => "string",
    };
    ArgInfo {
        name: a.get_id().to_string(),
        long: a.get_long().map(str::to_string),
        help: a.get_help().map(|s| s.to_string()).unwrap_or_default(),
        required: a.is_required_set(),
        kind: kind.to_string(),
        default: a.get_default_values().first().map(|v| v.to_string_lossy().into_owned()).filter(|v| v != "false"),
    }
}

/// Everything in an after_help text but the `Examples:` block (the heading and the lines up to
/// the first blank one), with the blank lines around it trimmed.
fn notes_of(after: &str) -> String {
    let mut in_examples = false;
    let mut kept = vec![];
    for l in after.lines() {
        if l.trim() == "Examples:" {
            in_examples = true;
        } else if in_examples && l.trim().is_empty() {
            in_examples = false;
        } else if !in_examples {
            kept.push(l);
        }
    }
    kept.join("\n").trim_matches('\n').to_string()
}
