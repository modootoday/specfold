//! Runs one command line to an output, without printing. The binary and the MCP server share it.

use crate::cli::args::{Cli, Cmd};
use crate::cli::dispatch::dispatch;
use crate::output::{Code, Format, Output, Page};
use clap::Parser;

pub struct Executed {
    pub output: Output,
    pub format: Format,
    pub page: Page,
    /// clap's own help or version text, printed as is.
    pub raw: Option<String>,
    pub serve_mcp: bool,
}

fn code_of(e: &anyhow::Error) -> Code {
    if e.chain().any(|c| c.downcast_ref::<std::io::Error>().is_some()) { Code::Env } else { Code::Usage }
}

/// The subcommand named on the command line, if it is a real one.
fn command_name(argv: &[String]) -> Option<String> {
    let first = argv.iter().skip(1).find(|a| !a.starts_with('-'))?;
    crate::cli::describe::commands().iter().any(|c| &c.name == first).then(|| first.clone())
}

fn help_hint(name: &Option<String>) -> String {
    match name {
        Some(n) => format!("spec help {n}"),
        None => "spec help".into(),
    }
}

pub fn execute(argv: Vec<String>) -> Executed {
    let page = Page { limit: 30, offset: 0 };
    let cli = match Cli::try_parse_from(&argv) {
        Ok(c) => c,
        Err(e) => {
            let wants_json = argv.iter().any(|a| a == "--json");
            let kind = e.kind();
            let display_only = matches!(
                kind,
                clap::error::ErrorKind::DisplayHelp
                    | clap::error::ErrorKind::DisplayVersion
                    | clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
            );
            let name = command_name(&argv);
            let rendered = e.render().to_string();
            if display_only && !wants_json {
                return Executed {
                    output: Output::new("help"),
                    format: Format::Text,
                    page,
                    raw: Some(rendered),
                    serve_mcp: false,
                };
            }
            let first = clap_message(&rendered);
            // A mistyped subcommand: the nearest real one is the next command.
            let name = name.or_else(|| nearest_command(&argv));
            let code = if display_only { Code::Ok } else { Code::Usage };
            let out = Output::error(name.as_deref().unwrap_or("spec"), code, &first, Some(help_hint(&name)));
            return Executed {
                output: out,
                format: if wants_json { Format::Json } else { Format::Text },
                page,
                raw: None,
                serve_mcp: false,
            };
        }
    };
    let format = if cli.global.json {
        Format::Json
    } else if cli.global.jsonl {
        Format::Jsonl
    } else {
        Format::Text
    };
    let page = Page { limit: cli.global.limit, offset: cli.global.offset };
    if matches!(cli.cmd, Cmd::Mcp) {
        return Executed { output: Output::new("mcp"), format, page, raw: None, serve_mcp: true };
    }
    let output = match dispatch(&cli) {
        Ok(o) => o,
        Err(e) => {
            let name = command_name(&argv);
            let (msg, next) = split_next(&format!("{e:#}"), &argv);
            Output::error(
                name.as_deref().unwrap_or("spec"),
                code_of(&e),
                &msg,
                Some(next.unwrap_or_else(|| help_hint(&name))),
            )
        }
    };
    Executed { output, format, page, raw: None, serve_mcp: false }
}

/// clap's error without its usage block: the first line, the missing arguments it lists under
/// it, and its tip, on one line.
fn clap_message(rendered: &str) -> String {
    let mut lines = rendered.lines();
    let mut msg = lines.next().unwrap_or("").trim_start_matches("error: ").trim_end_matches(':').to_string();
    let mut listed = vec![];
    let mut tip = None;
    for l in lines.take_while(|l| !l.starts_with("Usage:")) {
        let t = l.trim();
        if let Some(rest) = t.strip_prefix("tip: ") {
            tip = Some(rest.to_string());
        } else if !t.is_empty() && l.starts_with("  ") {
            listed.push(t.to_string());
        }
    }
    if !listed.is_empty() {
        msg = format!("{msg}: {}", listed.join(", "));
    }
    if let Some(t) = tip {
        msg = format!("{msg} ({t})");
    }
    msg
}

/// The real subcommand a mistyped first word is closest to.
fn nearest_command(argv: &[String]) -> Option<String> {
    let first = argv.iter().skip(1).find(|a| !a.starts_with('-'))?;
    let names: Vec<String> = crate::cli::describe::commands().into_iter().map(|c| c.name).collect();
    crate::commands::setup::help::nearest(first, names.iter().map(|s| s.as_str())).map(str::to_string)
}

fn quote(a: &str) -> String {
    if !a.is_empty() && a.chars().all(|c| c.is_ascii_alphanumeric() || "-_./:=@,+".contains(c)) {
        a.to_string()
    } else {
        format!("'{}'", a.replace('\'', "'\\''"))
    }
}

/// An error that names its own way out keeps it as the one next command: `; next: <cmd>` as is,
/// and `rerun with --flag` as the full command line with the flag added.
fn split_next(msg: &str, argv: &[String]) -> (String, Option<String>) {
    if let Some((m, n)) = msg.split_once("; next: ") {
        return (m.to_string(), Some(n.to_string()));
    }
    if let Some((m, flag)) = msg.rsplit_once("; rerun with ")
        && flag.starts_with("--")
        && !flag.contains(' ')
    {
        let mut cmd: Vec<String> =
            std::iter::once("spec".to_string()).chain(argv.iter().skip(1).map(|a| quote(a))).collect();
        cmd.push(flag.to_string());
        return (m.to_string(), Some(cmd.join(" ")));
    }
    (msg.to_string(), None)
}
