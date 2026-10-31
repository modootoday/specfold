//! Runs one command line to an output, without printing. The binary and the MCP server share it.

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

fn command_name(argv: &[String]) -> Option<String> {
    let first = first_word(argv)?;
    // Hidden aliases name the command they stand for.
    let first = match first.as_str() {
        "list" | "ls" => "find".to_string(),
        _ => first.clone(),
    };
    crate::cli::describe::commands().iter().any(|c| c.name == first).then_some(first)
}

fn help_hint(name: &Option<String>) -> String {
    match name {
        Some(n) => format!("spec help {n}"),
        None => "spec help".into(),
    }
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
            let json_fix = (kind == clap::error::ErrorKind::InvalidValue).then(|| format_json(&argv)).flatten();
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
                with_dirs(&mut out, &quote(&d), &quote(&d));
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
    // C192: -C is a chdir: relative paths, repository discovery and repo_path all start there.
    // The typed value stays for the hints; the session gets the absolute directory.
    let shown_dir = cli.global.dir.clone();
    if let Some(abs) = shown_dir.as_ref().and_then(|d| d.canonicalize().ok()).filter(|a| a.is_dir())
        && std::env::set_current_dir(&abs).is_ok()
    {
        cli.global.dir = Some(abs);
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
        with_dir(&mut output, d, &cli.global);
    }
    Executed { output, format, page, raw: None, serve_mcp: false }
}

/// C161/C163: commands to paste work from where -C was typed: spec gets `-C <dir>`, git gets
/// `git -C <dir>`, any other shell command `cd <dir> &&`.
fn with_dir(o: &mut Output, dir: &std::path::Path, g: &crate::cli::args::global::Global) {
    let w = specfold_core::shell::quote(&dir.display().to_string());
    // C192: undo and fix name files from the repository root, so shell commands run there.
    let abs = g.dir.clone().unwrap_or_default();
    let root = crate::commands::session::config_quiet(g).ok().map(|c| root_as_typed(dir, &abs, &c.root));
    let r = root.map(|r| specfold_core::shell::quote(&r.display().to_string())).unwrap_or_else(|| w.clone());
    with_dirs(o, &w, &r);
}

/// The repository root written the way -C was: `my repo/src` gives `my repo`, `.` when -C is
/// the root, otherwise the absolute root.
fn root_as_typed(dir: &std::path::Path, abs: &std::path::Path, root: &std::path::Path) -> std::path::PathBuf {
    let below = abs.strip_prefix(root).ok().map(|r| r.to_path_buf());
    match below {
        Some(b) if b.as_os_str().is_empty() => dir.to_path_buf(),
        Some(b) => {
            let mut up = dir.to_path_buf();
            for _ in b.components() {
                if !up.pop() {
                    return root.to_path_buf();
                }
            }
            if up.as_os_str().is_empty() { ".".into() } else { up }
        }
        None => root.to_path_buf(),
    }
}

fn with_dirs(o: &mut Output, w: &str, r: &str) {
    o.next = o.next.as_deref().map(|n| dir_cmd(n, w, r));
    if let Some(serde_json::Value::String(u)) = o.summary.get_mut("undo") {
        *u = dir_cmd(u, w, r);
    }
    for f in &mut o.foot {
        if let Some(u) = f.strip_prefix("undo: ") {
            *f = format!("undo: {}", dir_cmd(u, w, r));
        }
    }
    let spec_c = format!("spec -C {w} ");
    for it in &mut o.items {
        if let Some(serde_json::Value::String(c)) = it.json.get_mut("fix").and_then(|f| f.get_mut("cmd")) {
            *c = dir_cmd(c, w, r);
        }
        if it.text.contains("\n  fix: ") {
            it.text = it
                .text
                .lines()
                .map(|l| match l.strip_prefix("  fix: ") {
                    Some(c) => format!("  fix: {}", dir_cmd(c, w, r))
                        .replace("(spec fix --write", &format!("({spec_c}fix --write")),
                    None => l.to_string(),
                })
                .collect::<Vec<_>>()
                .join("\n");
        }
    }
}

/// Shell commands that next, undo and fix hints start with.
const SHELL: &[&str] = &["git", "mkdir", "mv", "ls", "rm", "rmdir", "cp", "cat", "touch", "ln", "find"];

fn dir_cmd(cmd: &str, w: &str, r: &str) -> String {
    let spec = |part: &str| match part.strip_prefix("spec ") {
        Some(rest) if !rest.starts_with("-C ") && !rest.starts_with("--dir") => format!("spec -C {w} {rest}"),
        _ => part.to_string(),
    };
    if cmd.starts_with("spec ") {
        return match cmd.split_once("# then: ") {
            Some((a, b)) => format!("{}# then: {}", spec(a), spec(b)),
            None => spec(cmd),
        };
    }
    let first = cmd.split_whitespace().next().unwrap_or("");
    // Prose such as "replace `x` with `y`" is not a command.
    if !SHELL.contains(&first) {
        return cmd.to_string();
    }
    if first == "git" && !cmd.contains("&&") && !cmd.starts_with("git -C ") {
        return format!("git -C {r} {}", &cmd[4..]);
    }
    format!("cd {r} && {cmd}")
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
        return (m.to_string(), Some(cmd.join(" ")));
    }
    if let Some(cmd) = trailing_command(msg) {
        return (msg.to_string(), Some(cmd));
    }
    (msg.to_string(), None)
}

/// A message that ends in a complete command (`...: spec new --translation ja --from x`) names
/// its own next step; a command with a `<placeholder>` in it is not runnable.
fn trailing_command(msg: &str) -> Option<String> {
    let (_, cmd) = msg.rsplit_once(": spec ")?;
    let cmd = format!("spec {}", cmd.trim_end_matches('.'));
    (!cmd.contains('<') && !cmd.contains("; ") && !cmd.contains(". ")).then_some(cmd)
}
