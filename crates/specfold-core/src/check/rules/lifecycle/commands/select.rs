//! Which documents a command run reaches, and which of their `verify` entries are commands.

use crate::check::context::Ctx;
use crate::corpus::graph::query;
use crate::model::Doc;
use crate::workspace::owners::matcher;

/// The marker for a rule that has no runnable check and is verified by people.
const MANUAL: &str = "manual";

pub fn verify_commands(d: &Doc) -> Vec<String> {
    d.list_field("verify").into_iter().filter(|c| !c.trim().eq_ignore_ascii_case(MANUAL)).collect()
}

fn is_gitlink(ctx: &Ctx, path: &str) -> bool {
    ctx.corpus.cfg.spec_home.join(path).join(".git").exists()
}

/// Every document for a full run or when a path in `full_run_on` changed; otherwise those
/// governing a changed path (a changed submodule reaches globs inside it), those whose command
/// names a changed file, and those edited themselves.
pub fn selected(ctx: &Ctx) -> Vec<usize> {
    let all = || (0..ctx.corpus.docs.len()).collect();
    if ctx.scope.is_all() {
        return all();
    }
    let full: Vec<_> = ctx.corpus.cfg.raw.verify.full_run_on.iter().filter_map(|p| matcher(p)).collect();
    let hit = ctx.scope.paths.iter().find(|p| full.iter().any(|m| m.is_match(p)) && !neutral_config_edit(ctx, p));
    if let Some(p) = hit {
        say_full_run(ctx, p);
        return all();
    }
    let mut out: Vec<usize> =
        ctx.scope.paths.iter().flat_map(|p| query::governing(ctx.corpus, p).into_iter().map(|g| g.idx)).collect();
    for (i, d) in ctx.corpus.docs.iter().enumerate() {
        let commands = verify_commands(d);
        let governs = d.rel().get("governs").cloned().unwrap_or_default();
        let reached = ctx.scope.paths.iter().any(|p| {
            commands.iter().any(|c| c.contains(p.as_str()))
                || (is_gitlink(ctx, p) && governs.iter().any(|g| crate::paths::is_below(g, p)))
        });
        if reached {
            out.push(i);
        }
    }
    out.extend(ctx.scope.touched.iter().copied());
    out.sort();
    out.dedup();
    out
}

/// Keys of spec.toml that never change which commands run or what they report (N3).
const NEUTRAL: &[&[&str]] = &[&["remotes"], &["prefix"], &["check", "baseline"]];

/// Whether `path` is the configuration file and its edit touches only neutral keys.
fn neutral_config_edit(ctx: &Ctx, path: &str) -> bool {
    if path != "spec.toml" {
        return false;
    }
    let parse = |t: Option<String>| t.and_then(|t| t.parse::<toml::Table>().ok()).map(strip);
    match (parse(ctx.previous(path)), parse(ctx.current(path))) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

fn strip(mut t: toml::Table) -> toml::Table {
    for keys in NEUTRAL {
        match keys {
            [k] => {
                t.remove(*k);
            }
            [s, k] => {
                if let Some(toml::Value::Table(sub)) = t.get_mut(*s) {
                    sub.remove(*k);
                }
            }
            _ => {}
        }
    }
    t
}

/// One line on stderr saying why every verify command runs, printed once per process.
fn say_full_run(ctx: &Ctx, path: &str) {
    static ONCE: std::sync::Once = std::sync::Once::new();
    if ctx.opts.run_commands {
        ONCE.call_once(|| eprintln!("spec: full verify run: {path} changed and matches [verify] full_run_on"));
    }
}
