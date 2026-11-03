//! A command line that does not parse, or a command that failed: which command it was about, the
//! message without clap's usage block, and the one next command the error names.

use super::dir_rewrite::with_dirs;
use super::run::Executed;
use crate::cli::usage::{format_json, typed_dir, usage_example};
use crate::output::{Code, Format, Output, Page};
use specfold_core::shell::quote;

/// A command line clap refused: its help or version text as is, or a usage error that names
/// the command meant and the one next command to try.
pub(super) fn parse_failure(argv: &[String], e: &clap::Error, page: Page) -> Executed {
    let wants_json = argv.iter().any(|a| a == "--json");
    let kind = e.kind();
    let display_only = matches!(
        kind,
        clap::error::ErrorKind::DisplayHelp
            | clap::error::ErrorKind::DisplayVersion
            | clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
    );
    let name = command_name(argv);
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
        .then(|| format_json(argv))
        .flatten();
    if json_fix.is_some() {
        first.push_str("; JSON output is --json");
    }
    // A mistyped subcommand: the nearest real one is the next command, but the error
    // keeps the generic schema rather than following the guess.
    let schema_name = name.clone();
    let name = name.or_else(|| nearest_command(argv));
    let code = if display_only { Code::Ok } else { Code::Usage };
    let next =
        json_fix.or_else(|| usage_example(argv, kind, &rendered, name.as_deref())).unwrap_or_else(|| help_hint(&name));
    let mut out = Output::error(schema_name.as_deref().unwrap_or("spec"), code, &first, Some(next));
    // C229: the next command runs where -C pointed, as on every other path.
    if let Some(d) = typed_dir(argv).filter(|d| !d.is_empty()) {
        with_dirs(&mut out, &quote(&d), &quote(&d), true);
    }
    Executed {
        output: out,
        format: if wants_json { Format::Json } else { Format::Text },
        page,
        raw: None,
        serve_mcp: false,
    }
}

/// A command that failed: exit code by cause, and the next command the error names, else the
/// config reference for a broken spec.toml, else the command's help.
pub(super) fn failed(argv: &[String], e: &anyhow::Error) -> Output {
    let name = command_name(argv);
    let (msg, next) = split_next(&format!("{e:#}"), argv);
    Output::error(
        name.as_deref().unwrap_or("spec"),
        code_of(e),
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

fn code_of(e: &anyhow::Error) -> Code {
    if e.chain().any(|c| c.downcast_ref::<std::io::Error>().is_some()) { Code::Env } else { Code::Usage }
}

/// C193: the first word that is not an option or the value of a global option that takes one
/// (`-C ''`, `--limit 5`), so `spec -C '' find` is about find.
fn first_word(argv: &[String]) -> Option<&String> {
    let mut it = argv.iter().skip(1);
    while let Some(a) = it.next() {
        if matches!(a.as_str(), "-C" | "--dir" | "--limit" | "--offset" | "--max-bytes") {
            it.next();
        } else if !a.starts_with('-') {
            return Some(a);
        }
    }
    None
}

pub(super) fn command_name(argv: &[String]) -> Option<String> {
    let first = first_word(argv)?;
    // Hidden aliases name the command they stand for.
    let first = match first.as_str() {
        "list" | "ls" => "find".to_string(),
        _ => first.clone(),
    };
    crate::cli::describe::commands().iter().any(|c| c.name == first).then_some(first)
}

pub(super) fn help_hint(name: &Option<String>) -> String {
    match name {
        Some(n) => format!("spec help {n}"),
        None => "spec help".into(),
    }
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
    let first = first_word(argv)?;
    let names: Vec<String> = crate::cli::describe::commands().into_iter().map(|c| c.name).collect();
    crate::commands::setup::help::nearest(first, names.iter().map(|s| s.as_str())).map(str::to_string)
}

/// An error that names its own way out keeps it as the one next command: `; next: <cmd>` as is,
/// and `rerun with --flag` as the full command line with the flag added.
fn split_next(msg: &str, argv: &[String]) -> (String, Option<String>) {
    if let Some((m, n)) = msg.split_once("; next: ") {
        // A wrapped error may name a second way out; the innermost one is the most specific.
        let n = n.split("; next: ").next().unwrap_or(n);
        return (m.to_string(), Some(n.to_string()));
    }
    if let Some((m, flag)) = msg.rsplit_once("; rerun with ")
        && flag.starts_with("--")
        && flag.split(' ').count() <= 2
        && flag.split(' ').nth(1).is_none_or(|v| !v.starts_with('-'))
    {
        // C156: a flag already on the line is replaced, not given twice.
        let name = flag.split(' ').next().unwrap_or(flag);
        let valued = flag.contains(' ');
        let mut kept = vec![];
        let mut it = argv.iter().skip(1);
        while let Some(a) = it.next() {
            if a == name {
                if valued {
                    it.next();
                }
                continue;
            }
            if a.starts_with(&format!("{name}=")) {
                continue;
            }
            kept.push(quote(a));
        }
        let mut cmd: Vec<String> = std::iter::once("spec".to_string()).chain(kept).collect();
        cmd.extend(flag.split(' ').map(str::to_string));
        if previewed_write(argv) {
            cmd.push("--write".into());
        }
        return (m.to_string(), Some(cmd.join(" ")));
    }
    if let Some(cmd) = trailing_command(msg) {
        return (msg.to_string(), Some(cmd));
    }
    (msg.to_string(), None)
}

/// A write command run as a preview (no --write, no --dry-run): its rerun hint is the step that
/// writes, so it keeps the write the preview would have offered next.
fn previewed_write(argv: &[String]) -> bool {
    if argv.iter().any(|a| a == "--write" || a == "--dry-run") {
        return false;
    }
    let Some(first) = first_word(argv) else { return false };
    crate::cli::describe::commands()
        .into_iter()
        .find(|c| &c.name == first || c.aliases.iter().any(|a| a == first))
        .is_some_and(|c| c.args.iter().any(|a| a.long.as_deref() == Some("write")))
}

/// A message that ends in a complete command (`...: spec new --translation ja --from x`) names
/// its own next step; a command with a `<placeholder>` in it is not runnable.
fn trailing_command(msg: &str) -> Option<String> {
    let (_, cmd) = msg.rsplit_once(": spec ")?;
    let cmd = format!("spec {}", cmd.trim_end_matches('.'));
    (!cmd.contains('<') && !cmd.contains("; ") && !cmd.contains(". ")).then_some(cmd)
}
