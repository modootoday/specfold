//! Runs one command line to an output, without printing. The binary and the MCP server share it.

use super::dir_rewrite::{with_dir, with_dirs};
use super::usage_errors::{clap_message, command_name, help_hint, nearest_command, split_next};
use crate::cli::args::{Cli, Cmd};
use crate::cli::dispatch::dispatch;
use crate::cli::usage::{format_json, typed_dir, usage_example};
use crate::output::{Code, Format, Output, Page};
use clap::FromArgMatches;
use specfold_core::shell::quote;

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
fn dir_is_file(cli: &Cli, argv: &[String]) -> Option<Output> {
    let d = cli.global.dir.as_ref().filter(|d| d.exists() && !d.is_dir())?;
    let name = command_name(argv);
    let msg = format!("cannot open {} as a directory: it is a file", d.display());
    Some(Output::error(name.as_deref().unwrap_or("spec"), Code::Env, &msg, Some(help_hint(&name))))
}

/// CLI-12: `--all` before any command (`spec --help --all`) lists every command; it is taken
/// out so clap does not read it as an unknown flag.
fn top_level_all(argv: Vec<String>) -> (Vec<String>, bool) {
    let before_cmd: Vec<&String> = argv.iter().skip(1).take_while(|a| a.starts_with('-')).collect();
    if !before_cmd.iter().any(|a| *a == "--all") {
        return (argv, false);
    }
    let mut seen = false;
    let out = argv
        .into_iter()
        .enumerate()
        .filter(|(i, a)| {
            let drop = !seen && *i > 0 && a == "--all";
            seen |= drop;
            !drop
        })
        .map(|(_, a)| a)
        .collect();
    (out, true)
}

/// C215: arguments as UTF-8, or a usage error naming the first one that is not (no panic).
pub fn utf8_args(argv: Vec<std::ffi::OsString>) -> Result<Vec<String>, Box<Executed>> {
    let wants_json = argv.iter().any(|a| a == "--json");
    let mut out = Vec::with_capacity(argv.len());
    for (i, a) in argv.into_iter().enumerate() {
        match a.into_string() {
            Ok(s) => out.push(s),
            Err(bad) => {
                let msg = format!(
                    "argument {i} ({}) is not valid UTF-8; rename it, or pass a directory above it",
                    bad.to_string_lossy()
                );
                let next = "spec help".to_string();
                let output = Output::error("spec", Code::Usage, &msg, Some(next));
                let format = if wants_json { Format::Json } else { Format::Text };
                let page = Page { limit: 30, offset: 0, max_bytes: None };
                return Err(Box::new(Executed { output, format, page, raw: None, serve_mcp: false }));
            }
        }
    }
    Ok(out)
}

pub fn execute(argv: Vec<String>) -> Executed {
    let page = Page { limit: 30, offset: 0, max_bytes: None };
    let (argv, warning) = crate::cli::describe::undeprecate(&argv, crate::cli::describe::DEPRECATED);
    if let Some(w) = warning {
        eprintln!("{w}");
    }
    if let Some(name) = command_name(&argv) {
        for w in crate::cli::describe::flag_warnings(&name, &argv) {
            eprintln!("{w}");
        }
    }
    let (argv, all) = top_level_all(argv);
    let parsed =
        crate::cli::describe::parser_command(all).try_get_matches_from(&argv).and_then(|m| Cli::from_arg_matches(&m));
    let mut cli = match parsed {
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
            let mut first = clap_message(&rendered);
            // C241: only when --format is the bad value; a missing value elsewhere is not fixed by --json.
            let json_fix = (kind == clap::error::ErrorKind::InvalidValue && rendered.contains("for '--format"))
                .then(|| format_json(&argv))
                .flatten();
            if json_fix.is_some() {
                first.push_str("; JSON output is --json");
            }
            // A mistyped subcommand: the nearest real one is the next command, but the error
            // keeps the generic schema rather than following the guess.
            let schema_name = name.clone();
            let name = name.or_else(|| nearest_command(&argv));
            let code = if display_only { Code::Ok } else { Code::Usage };
            let next = json_fix
                .or_else(|| usage_example(&argv, kind, &rendered, name.as_deref()))
                .unwrap_or_else(|| help_hint(&name));
            let mut out = Output::error(schema_name.as_deref().unwrap_or("spec"), code, &first, Some(next));
            // C229: the next command runs where -C pointed, as on every other path.
            if let Some(d) = typed_dir(&argv).filter(|d| !d.is_empty()) {
                with_dirs(&mut out, &quote(&d), &quote(&d), true);
            }
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
    let page = Page { limit: cli.global.limit, offset: cli.global.offset, max_bytes: cli.global.max_bytes };
    if matches!(cli.cmd, Cmd::Lsp) {
        let code = match crate::lsp::server::serve(cli.global) {
            Ok(()) => Code::Ok,
            Err(_) => Code::Env,
        };
        return Executed {
            output: Output::new("lsp").code(code),
            format,
            page,
            raw: Some(String::new()),
            serve_mcp: false,
        };
    }
    if matches!(cli.cmd, Cmd::Mcp) {
        return Executed { output: Output::new("mcp"), format, page, raw: None, serve_mcp: true };
    }
    // C193: -C names a directory; a file is refused like a missing one, and with no -C in the
    // hint, which would only repeat the problem.
    if let Some(output) = dir_is_file(&cli, &argv) {
        return Executed { output, format, page, raw: None, serve_mcp: false };
    }
    let shown_dir = cli.global.dir.clone();
    let loc = enter(&mut cli.global);
    let output = match dispatch(&cli) {
        Ok(o) => o,
        Err(e) => {
            let name = command_name(&argv);
            let (msg, next) = split_next(&format!("{e:#}"), &argv);
            Output::error(
                name.as_deref().unwrap_or("spec"),
                code_of(&e),
                &msg,
                // C106: a broken spec.toml is fixed from the config reference, not the command's help.
                Some(next.unwrap_or_else(|| {
                    if msg.starts_with("invalid ") && msg.contains("spec.toml") {
                        "spec explain config".into()
                    } else {
                        help_hint(&name)
                    }
                })),
            )
        }
    };
    let mut output = output;
    if let Some(d) = &shown_dir {
        with_dir(&mut output, d, &loc);
    } else if let Some(up) = loc.up_to_root() {
        // C262: from a subfolder, shell commands in undo and next still name root paths.
        with_dirs(&mut output, "", &specfold_core::shell::quote(&up), false);
    }
    Executed { output, format, page, raw: None, serve_mcp: false }
}

/// C192: -C is a chdir: relative paths, repository discovery and repo_path all start there.
/// The typed value stays for the hints; the command gets the absolute directory and its location.
fn enter(g: &mut crate::cli::args::global::Global) -> std::sync::Arc<crate::commands::session::Location> {
    let shown_dir = g.dir.clone();
    if let Some(abs) = shown_dir.as_ref().and_then(|d| d.canonicalize().ok()).filter(|a| a.is_dir())
        && std::env::set_current_dir(&abs).is_ok()
    {
        g.dir = Some(abs);
    }
    let loc = crate::commands::session::discover(g, shown_dir);
    g.loc = Some(loc.clone());
    loc
}
