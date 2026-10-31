//! Runnable next commands for usage errors clap reports (C133, C165, C229).

use crate::cli::args::Cli;
use specfold_core::shell::quote;

/// A runnable command for a usage error the command line can be repaired from: a missing
/// document filled with a real id, `spec new` with its title.
pub(crate) fn usage_example(
    argv: &[String],
    kind: clap::error::ErrorKind,
    rendered: &str,
    name: Option<&str>,
) -> Option<String> {
    use clap::error::ErrorKind;
    let name = name?;
    let pos = argv.iter().position(|a| a == name)?;
    let rest: Vec<String> = argv[pos + 1..].iter().map(|a| quote(a)).collect();
    match (name, kind) {
        ("new", ErrorKind::UnknownArgument | ErrorKind::MissingRequiredArgument)
            if !argv.iter().any(|a| a == "--translation" || a.starts_with("--translation=")) =>
        {
            // C133: an option's value is not a positional word; --title and --governs carry over.
            let (words, opts) = split_new_args(&argv[pos + 1..]);
            // C178: an option `new` does not have would be carried into the next and fail the
            // same way; the caller falls back to `spec help new` instead.
            if opts.iter().any(|(k, _)| !new_knows(k)) {
                return None;
            }
            let ty = words.first().map(|t| t.as_str()).unwrap_or("adr");
            let given = opts.iter().find(|(k, _)| k == "--title").and_then(|(_, v)| v.clone());
            let title = match given {
                Some(t) => t,
                None if words.len() > 1 => words[1..].join(" "),
                None => "Use one payment provider".to_string(),
            };
            let mut cmd = format!("spec new {} {}", quote(ty), specfold_core::shell::opt("--title", &title));
            // C165: everything else the user gave (--from, --governs, --write, ...) carries over.
            for (k, v) in opts.iter().filter(|(k, _)| k != "--title" && k != "--json") {
                cmd.push(' ');
                // C229: a value starting with `-` is written `--opt=value`.
                match v {
                    Some(v) => cmd.push_str(&specfold_core::shell::opt(k, v)),
                    None => cmd.push_str(k),
                }
            }
            Some(cmd)
        }
        (_, ErrorKind::MissingRequiredArgument) if rendered.contains("<DOC>") && !rendered.contains("<DOC> <") => {
            let tasks = argv.iter().any(|a| a == "--tasks");
            let id = crate::commands::session::example_doc_for(argv, tasks.then_some("plan"))?;
            let mut cmd = vec!["spec".to_string(), name.to_string(), quote(&id)];
            cmd.extend(rest);
            Some(cmd.join(" "))
        }
        _ => None,
    }
}

/// Whether `spec new` (or the global options) defines this flag.
fn new_knows(flag: &str) -> bool {
    use clap::CommandFactory;
    let cmd = Cli::command();
    let long = flag.trim_start_matches('-');
    let short = (flag.len() == 2 && !flag.starts_with("--")).then(|| flag.chars().nth(1)).flatten();
    let all: Vec<_> =
        cmd.find_subcommand("new").into_iter().flat_map(|c| c.get_arguments()).chain(cmd.get_arguments()).collect();
    all.iter().any(|a| a.get_long() == Some(long) || (short.is_some() && a.get_short() == short))
}

/// The positional words and the options of `spec new` arguments in order (a switch has no
/// value), using the command's own definition to tell value-taking options from switches.
fn split_new_args(args: &[String]) -> (Vec<String>, Vec<(String, Option<String>)>) {
    use clap::CommandFactory;
    let cmd = Cli::command();
    let new = cmd.find_subcommand("new");
    let takes = |flag: &str| {
        let long = flag.trim_start_matches('-');
        let all = new.into_iter().flat_map(|c| c.get_arguments()).chain(cmd.get_arguments());
        all.filter(|a| a.get_long() == Some(long) || (flag.len() == 2 && a.get_short() == flag.chars().nth(1)))
            .any(|a| a.get_action().takes_values())
    };
    let (mut words, mut opts) = (vec![], vec![]);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if let Some((k, v)) = a.split_once('=').filter(|_| a.starts_with("--")) {
            opts.push((k.to_string(), Some(v.to_string())));
        } else if a.starts_with('-') && a.len() > 1 {
            if !takes(a) {
                opts.push((a.clone(), None));
            } else if let Some(v) = it.next() {
                opts.push((a.clone(), Some(v.clone())));
            }
        } else {
            words.push(a.clone());
        }
    }
    (words, opts)
}

/// `--format json` on a command whose formats are drawings: the same command with `--json`.
pub(crate) fn format_json(argv: &[String]) -> Option<String> {
    let mut out = vec!["spec".to_string()];
    let mut hit = false;
    let mut it = argv.iter().skip(1).peekable();
    while let Some(a) = it.next() {
        if a == "--format" && it.peek().is_some_and(|v| v.as_str() == "json") {
            it.next();
            hit = true;
            out.push("--json".into());
        } else if a == "--format=json" {
            hit = true;
            out.push("--json".into());
        } else if a == "--json" {
        } else {
            out.push(quote(a));
        }
    }
    hit.then(|| out.join(" "))
}

/// C229: the -C/--dir value typed before the command, so a usage error's next keeps it.
pub(crate) fn typed_dir(argv: &[String]) -> Option<String> {
    let mut it = argv.iter().skip(1);
    while let Some(a) = it.next() {
        if a == "-C" || a == "--dir" {
            return it.next().cloned();
        }
        if let Some(d) = a.strip_prefix("--dir=").or_else(|| a.strip_prefix("-C").filter(|d| !d.is_empty())) {
            return Some(d.to_string());
        }
        if matches!(a.as_str(), "--limit" | "--offset" | "--max-bytes") {
            it.next();
        } else if !a.starts_with('-') {
            return None;
        }
    }
    None
}
