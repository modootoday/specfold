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
            let first = rendered.lines().next().unwrap_or("").trim_start_matches("error: ").to_string();
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
            Output::error(name.as_deref().unwrap_or("spec"), code_of(&e), &format!("{e:#}"), Some(help_hint(&name)))
        }
    };
    Executed { output, format, page, raw: None, serve_mcp: false }
}
