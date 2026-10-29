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
                let near = nearest(name, all.iter().map(|c| c.name.as_str()));
                let hint = near.map(|n| format!("; did you mean '{n}'? (spec help {n})")).unwrap_or_default();
                bail!("no command '{name}'{hint}");
            }
            c
        }
        None => all,
    };
    if a.llms {
        return Ok(llms(&picked));
    }
    if a.markdown {
        return Ok(match (&a.command, picked.as_slice()) {
            (Some(_), [one]) => reference(one),
            _ => markdown(&picked),
        });
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

/// The README command table: one row per command, from the same data as --json. It opens with
/// a blank line: a table right under the `<!-- spec:commands -->` comment is read by kramdown as
/// part of an HTML block and not rendered.
fn markdown(cmds: &[describe::CommandInfo]) -> Output {
    let mut o =
        Output::new("help").unpaged().head("").head("| command | what it does |").head("| ------- | ------------ |");
    for c in cmds {
        let about = c.about.trim_end_matches('.').replace('|', "\\|");
        o = o.item(to_value(c).unwrap_or_default(), format!("| `spec {}` | {about} |", c.name));
    }
    o
}

/// llms.txt (llmstxt.org): H1, the package description as the summary, then link lists. Built
/// from the same data as the README table, so the two cannot disagree.
fn llms(cmds: &[describe::CommandInfo]) -> Output {
    let mut o = Output::new("help")
        .unpaged()
        .head("# specfold")
        .head("")
        .head(format!("> {}", env!("CARGO_PKG_DESCRIPTION")))
        .head("")
        .head("specfold is a command-line tool, `spec`. Every command takes `--help` and `--json`; `spec help --json` describes them all.")
        .head("")
        .head("## Commands")
        .head("");
    for c in cmds {
        o = o.item(
            to_value(c).unwrap_or_default(),
            format!("- [spec {}](docs/reference/commands.md): {}", c.name, cell(&c.about)),
        );
    }
    o.foot("")
        .foot("## Docs")
        .foot("")
        .foot("- [README](README.md): install, quick start and concepts")
        .foot("- [Concepts](docs/concepts/index.md): types, ids, statuses and relations")
        .foot("- [Agent assets](docs/guides/agents.md): skills, rules, subagents and instruction files")
        .foot("")
        .foot("## Optional")
        .foot("")
        .foot("- [Source code](https://github.com/modootoday/specfold)")
        .foot("- [Releases](https://github.com/modootoday/specfold/releases/latest)")
}

fn cell(s: &str) -> String {
    s.trim_end_matches('.').replace('|', "\\|").replace('\n', " ")
}

/// One command's reference page: the table row's wording, then its arguments and examples.
/// It opens with a blank line for the same reason the table does.
fn reference(c: &describe::CommandInfo) -> Output {
    let mut o = Output::new("help")
        .unpaged()
        .head("")
        .head(format!("## `spec {}`", c.name))
        .head("")
        .head(format!("{}.", cell(&c.about)));
    if !c.args.is_empty() {
        o = o.head("").head("| argument | what it does |").head("| -------- | ------------ |");
        for arg in &c.args {
            let flag = arg.long.as_ref().map(|l| format!("--{l}")).unwrap_or_else(|| format!("<{}>", arg.name));
            let req = if arg.required { " (required)" } else { "" };
            o = o.head(format!("| `{flag}` | {}{req} |", cell(&arg.help)));
        }
    }
    if !c.examples.is_empty() {
        o = o.head("").head("```sh");
        for e in &c.examples {
            o = o.head(e.clone());
        }
        o = o.head("```");
    }
    o.item(to_value(c).unwrap_or_default(), String::new())
}

/// The closest name by edit distance, if it is close enough to be a typo.
pub fn nearest<'a>(want: &str, names: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let dist = |a: &str, b: &str| {
        let b: Vec<char> = b.chars().collect();
        let mut row: Vec<usize> = (0..=b.len()).collect();
        for (i, ca) in a.chars().enumerate() {
            let mut prev = row[0];
            row[0] = i + 1;
            for j in 0..b.len() {
                let cur = row[j + 1];
                row[j + 1] = (prev + usize::from(ca != b[j])).min(row[j] + 1).min(cur + 1);
                prev = cur;
            }
        }
        row[b.len()]
    };
    names
        .map(|n| (dist(want, n), n))
        .filter(|(d, n)| *d <= 2.max(n.len() / 3) || n.starts_with(want))
        .min_by_key(|(d, _)| *d)
        .map(|(_, n)| n)
}
