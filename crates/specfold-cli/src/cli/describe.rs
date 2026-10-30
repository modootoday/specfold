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
    /// The schema name of its --json output.
    pub output: String,
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
    let examples = c
        .get_after_help()
        .map(|h| h.to_string().lines().map(str::trim).filter(|l| l.starts_with("spec ")).map(str::to_string).collect())
        .unwrap_or_default();
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
        output: format!("spec.{}.v1", c.get_name()),
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
