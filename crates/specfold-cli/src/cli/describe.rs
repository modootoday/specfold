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
    /// CLI-12: listed by the default `spec --help` and `spec help`; the rest need `--all`.
    pub core: bool,
    /// CLI-13: old names that still run this command, with a warning, until they are removed.
    pub deprecated_aliases: Vec<Deprecated>,
}

/// CLI-12: the commands the default help lists (at most 12); `--all` lists every command.
pub const CORE: &[&str] =
    &["context", "find", "show", "graph", "new", "set", "edit", "check", "guard", "fix", "explain", "init"];

/// CLI-13: a renamed command's old name. It stays a hidden alias that warns on stderr for at least
/// one minor release after `since`.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Deprecated {
    pub name: &'static str,
    #[serde(rename = "use")]
    pub replacement: &'static str,
    pub since: &'static str,
}

/// CLI-13: every deprecated command name. Empty while nothing has been renamed.
pub const DEPRECATED: &[Deprecated] = &[];

/// CLI-13: argv with a deprecated command name (the first non-flag argument) replaced, and the
/// warning to print. Only the command position is rewritten.
pub fn undeprecate(argv: &[String], table: &[Deprecated]) -> (Vec<String>, Option<String>) {
    let mut out = argv.to_vec();
    let Some(i) = argv.iter().enumerate().skip(1).find(|(_, a)| !a.starts_with('-')).map(|(i, _)| i) else {
        return (out, None);
    };
    match table.iter().find(|d| d.name == argv[i]) {
        Some(d) => {
            out[i] = d.replacement.to_string();
            let w = format!(
                "warning: `spec {}` is deprecated since {} and will be removed; use `spec {}`",
                d.name, d.since, d.replacement
            );
            (out, Some(w))
        }
        None => (out, None),
    }
}

/// CLI-12: the clap command the parser uses: commands outside CORE are hidden from the top-level
/// help unless `all`. Hidden commands still run and have their own `--help`.
pub fn parser_command(all: bool) -> clap::Command {
    let mut cmd = Cli::command();
    if !all {
        cmd = cmd.mut_subcommands(|c| if CORE.contains(&c.get_name()) { c } else { c.hide(true) });
        let more = cmd.get_subcommands().count() - CORE.len();
        cmd = cmd.after_help(format!("{more} more commands: spec --help --all (or spec help --all)"));
    }
    cmd
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
        core: CORE.contains(&c.get_name()),
        deprecated_aliases: DEPRECATED.iter().filter(|d| d.replacement == c.get_name()).copied().collect(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_deprecated_name_runs_the_new_command_and_warns() {
        let table = [Deprecated { name: "ls-files", replacement: "files", since: "0.3" }];
        let argv: Vec<String> = ["spec", "--json", "ls-files", "x"].iter().map(|s| s.to_string()).collect();
        let (out, w) = undeprecate(&argv, &table);
        assert_eq!(out, ["spec", "--json", "files", "x"]);
        assert!(w.unwrap().contains("use `spec files`"));
        let argv: Vec<String> = ["spec", "find", "ls-files"].iter().map(|s| s.to_string()).collect();
        assert_eq!(undeprecate(&argv, &table), (argv.clone(), None), "only the command position");
    }

    #[test]
    fn core_commands_exist_and_are_at_most_twelve() {
        let names: Vec<_> = Cli::command().get_subcommands().map(|c| c.get_name().to_string()).collect();
        assert!(CORE.len() <= 12);
        assert!(CORE.iter().all(|c| names.iter().any(|n| n == c)), "{CORE:?}");
        assert!(DEPRECATED.iter().all(|d| !names.iter().any(|n| n == d.name)), "an old name is not a command");
    }
}
