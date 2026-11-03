//! C161/C163/C192: the commands an output suggests (next, undo, fixes) rewritten to run from where
//! -C pointed, or from the repository root when spec ran in a subfolder.

use crate::output::Output;

/// C161/C163: commands to paste work from where -C was typed: spec gets `-C <dir>`, git gets
/// `git -C <dir>`, any other shell command `cd <dir> &&`.
pub(super) fn with_dir(o: &mut Output, dir: &std::path::Path, loc: &crate::commands::session::Location) {
    let w = specfold_core::shell::quote(&dir.display().to_string());
    // C192: undo and fix name files from the repository root, so shell commands run there.
    let root = loc.root_as_typed();
    let r = root.map(|r| specfold_core::shell::quote(&r.display().to_string())).unwrap_or_else(|| w.clone());
    with_dirs(o, &w, &r, true);
}

/// `spec` commands get `-C <w>` only when `spec_too` (from a subfolder spec finds the root itself).
pub(super) fn with_dirs(o: &mut Output, w: &str, r: &str, spec_too: bool) {
    let dir_cmd = |c: &str, w: &str, r: &str| {
        if !spec_too && c.starts_with("spec ") { c.to_string() } else { dir_cmd(c, w, r) }
    };
    o.next = o.next.as_deref().map(|n| dir_cmd(n, w, r));
    if let Some(serde_json::Value::String(u)) = o.summary.get_mut("undo") {
        *u = dir_cmd(u, w, r);
    }
    for f in &mut o.foot {
        if let Some(u) = f.strip_prefix("undo: ") {
            *f = format!("undo: {}", dir_cmd(u, w, r));
        }
    }
    let spec_c = if spec_too { format!("spec -C {w} ") } else { "spec ".into() };
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
