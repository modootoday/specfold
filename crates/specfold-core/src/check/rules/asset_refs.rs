//! A3: what agent assets point at still exists. Agents follow the paths and `spec` commands an
//! instruction file or skill names; a renamed file or a removed command keeps being tried.

use crate::assets;
use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

pub struct AssetRefsExist;

fn code_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"`([^`\n]+)`").unwrap())
}

fn link_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\]\(([^)\s]+)\)").unwrap())
}

/// A token that names a file in the repository, cleaned of anchors and punctuation; None for
/// URLs, ids, globs, placeholders, options and anything else that is not a plain path.
fn path_token(raw: &str, linked: bool) -> Option<&str> {
    let t = raw.trim().trim_end_matches(['.', ',', ';']);
    let t = t.split('#').next()?;
    if t.is_empty() || t.contains(char::is_whitespace) || t.starts_with('-') {
        return None;
    }
    if t.contains(['*', '?', '<', '>', '{', '}', '$', '~', '|', ':', '=', '@', '(', '[', '"', '\'']) {
        return None;
    }
    if t.contains("...") || t.starts_with("//") {
        return None;
    }
    if !linked && !t.contains('/') {
        return None;
    }
    // A git ref such as `origin/main` or `refs/heads/x` names a branch, not a file.
    if !linked && ["origin/", "upstream/", "refs/"].iter().any(|p| t.starts_with(p)) {
        return None;
    }
    Some(t)
}

fn exists(root: &Path, dir: &Path, t: &str) -> bool {
    if let Some(abs) = t.strip_prefix('/') {
        return root.join(abs).exists();
    }
    dir.join(t).exists() || root.join(t).exists()
}

/// The first word after `spec` in a command an asset names.
fn spec_subcommand(code: &str) -> Option<&str> {
    let code = code.trim().trim_start_matches("$ ").trim_start();
    let rest = code.strip_prefix("spec ")?;
    let word = rest.split_whitespace().next()?;
    (!word.starts_with('-') && word.chars().all(|c| c.is_ascii_lowercase() || c == '-')).then_some(word)
}

impl Rule for AssetRefsExist {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "A3",
            title: "paths and spec commands in agent assets exist",
            explain: "Agents act on what their instruction files and skills name. In every asset declared under [[assets]], a backticked path with a slash or a relative Markdown link must name a file or directory, looked up beside the asset and then from the repository root, and a `spec <command>` (inline or on a line of a code block) must be a command this version has. URLs, ids, globs, git refs such as origin/main and placeholders such as <id> are not read as paths. Fix the asset, or restore what it names. Reported as warnings.",
            on_commit: false,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        let root = &ctx.corpus.cfg.root;
        let commands = &ctx.opts.commands;
        for a in assets::load(&ctx.corpus.cfg) {
            let dir = root.join(&a.path).parent().map(Path::to_path_buf).unwrap_or_else(|| root.clone());
            let mut fenced = false;
            for (i, line) in a.body().lines().enumerate() {
                let n = a.body_line + i;
                if line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~") {
                    fenced = !fenced;
                    continue;
                }
                let mut cmds: Vec<&str> = vec![];
                if fenced {
                    cmds.extend(spec_subcommand(line));
                } else {
                    for c in code_re().captures_iter(line) {
                        let code = c.get(1).map_or("", |m| m.as_str());
                        if let Some(w) = spec_subcommand(code) {
                            cmds.push(w);
                        } else if let Some(t) = path_token(code, false)
                            && !exists(root, &dir, t)
                        {
                            out.push(Finding::warn("A3", &a.path, n, format!("names {t}, which does not exist")));
                        }
                    }
                    for c in link_re().captures_iter(line) {
                        let target = c.get(1).map_or("", |m| m.as_str());
                        if let Some(t) = path_token(target, true)
                            && !exists(root, &dir, t)
                        {
                            out.push(Finding::warn("A3", &a.path, n, format!("links to {t}, which does not exist")));
                        }
                    }
                }
                if commands.is_empty() {
                    continue;
                }
                for w in cmds {
                    if !commands.iter().any(|c| c == w) {
                        out.push(
                            Finding::warn("A3", &a.path, n, format!("runs `spec {w}`, which is not a spec command"))
                                .fix_cmd("spec help"),
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens() {
        assert_eq!(path_token("docs/setup.md", false), Some("docs/setup.md"));
        assert_eq!(path_token("docs/setup.md#install", false), Some("docs/setup.md"));
        assert_eq!(path_token("README.md", false), None);
        assert_eq!(path_token("README.md", true), Some("README.md"));
        assert_eq!(path_token("https://x.io/a", true), None);
        assert_eq!(path_token("sot:shop/cart", false), None);
        assert_eq!(path_token("src/**/*.rs", false), None);
        assert_eq!(path_token("spec show <id>", false), None);
        assert_eq!(path_token("origin/main", false), None);
        assert_eq!(path_token("origin/feat/x", false), None);
        assert_eq!(path_token("refs/heads/main", false), None);
        assert_eq!(path_token("origin/notes.md", true), Some("origin/notes.md"));
        assert_eq!(spec_subcommand("spec check --staged"), Some("check"));
        assert_eq!(spec_subcommand("$ spec show x"), Some("show"));
        assert_eq!(spec_subcommand("spec --json"), None);
        assert_eq!(spec_subcommand("specfold x"), None);
    }
}
