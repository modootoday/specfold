//! V3: external sources (an agent's memory, instruction files, notes) name documents and paths
//! that exist. Citations are checked with --external; `check_paths` and `ids` turn on, per
//! source, the check of backticked repository paths and of every document id.

use super::asset_refs::{code_re, path_token};
use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::config::Config;
use crate::config::types::ExternalRoot;
use crate::corpus::graph::Resolution;
use crate::model::markdown::body;
use crate::workspace::{vcs, walk};
use regex::Regex;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub struct ExternalCites;

/// `type:owner/key`, optionally `#anchor`, not inside a longer word, URL or remote reference.
fn id_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"(?:^|[^A-Za-z0-9_:/.-])([a-z][a-z0-9-]*):([A-Za-z0-9_.-]+/[A-Za-z0-9_.-]*[A-Za-z0-9_-](?:#[A-Za-z0-9_-]+)?)")
            .expect("static regex is valid")
    })
}

/// The Markdown files of one source: the file itself, or every `.md` file under the directory.
fn files(cfg: &Config, base: &Path) -> Vec<PathBuf> {
    if base.is_file() {
        return vec![base.to_path_buf()];
    }
    walk(cfg, base)
        .standard_filters(false)
        .build()
        .flatten()
        .filter(|e| e.file_type().is_some_and(|t| t.is_file()) && e.path().extension().is_some_and(|x| x == "md"))
        .map(|e| e.into_path())
        .collect()
}

/// What a finding names, `<source>:<path below the source>` (the file name for a source that is
/// one file), and the repository path when the file lies in the repository.
fn shown(cfg: &Config, name: &str, base: &Path, file: &Path) -> (String, Option<String>) {
    let below = file.strip_prefix(base).ok().filter(|p| !p.as_os_str().is_empty());
    let below = below.unwrap_or_else(|| Path::new(file.file_name().unwrap_or_default()));
    (format!("{name}:{}", crate::paths::slash(below)), crate::paths::rel_of(&cfg.spec_home, file))
}

/// Whether a backticked repository path exists: on disk, or in the commit a gitlink pins (also
/// when that cannot be read, so a submodule not checked out raises nothing).
fn path_exists(root: &Path, links: &[String], t: &str) -> bool {
    let t = t.trim_start_matches('/');
    if root.join(t).exists() {
        return true;
    }
    match links.iter().find(|g| crate::paths::is_below(t, g)) {
        Some(g) => vcs::gitlink_holds(root, g, &t[g.len() + 1..]) != Some(false),
        None => false,
    }
}

/// One file's findings, each (line, message) once.
struct Seen<'a> {
    path: &'a str,
    done: BTreeSet<(usize, String)>,
    out: Vec<Finding>,
}

impl Seen<'_> {
    fn warn(&mut self, line: usize, msg: String) -> Option<&mut Finding> {
        if !self.done.insert((line, msg.clone())) {
            return None;
        }
        self.out.push(Finding::warn("V3", self.path, line, msg));
        self.out.last_mut()
    }
}

/// The checks `--external` asks of every source: `[[references]]` and spec-path citations.
fn citations(ctx: &Ctx, text: &str, seen: &mut Seen) {
    let scan = body::scan(text, 1, &ctx.corpus.cfg.path_cite_re());
    for l in scan.links.iter().filter(|l| l.text.contains(':') || l.text.contains('/')) {
        if matches!(ctx.graph.resolve(&l.text), Resolution::Missing | Resolution::MissingAnchor { .. }) {
            seen.warn(l.line, format!("[[{}]] names no document", l.text));
        }
    }
    for c in &scan.path_cites {
        match ctx.graph.resolve(&c.text) {
            Resolution::Doc { idx, .. } => {
                let id = format!("[[{}]]", ctx.corpus.docs[idx].id);
                if let Some(f) = seen.warn(c.line, format!("cites the path {}", c.text)) {
                    *f = f.clone().fix_replace(c.text.clone(), id);
                }
            }
            _ => {
                seen.warn(c.line, format!("cites {} which does not exist", c.text));
            }
        }
    }
}

/// `ids` and `check_paths`: every id-shaped token of a known type resolves, and every backticked
/// repository path outside code blocks exists.
fn facts(ctx: &Ctx, ext: &ExternalRoot, links: &[String], text: &str, seen: &mut Seen) {
    let cfg = &ctx.corpus.cfg;
    let mut fence = crate::model::markdown::Fence::default();
    for (i, line) in text.lines().enumerate() {
        let n = i + 1;
        if fence.step(line) {
            continue;
        }
        if ext.ids {
            for c in id_re().captures_iter(line) {
                let (Some(ty), Some(rest)) = (c.get(1), c.get(2)) else { continue };
                if cfg.registry.get(ty.as_str()).is_none() {
                    continue;
                }
                let id = format!("{}:{}", ty.as_str(), rest.as_str().trim_end_matches('.'));
                if matches!(ctx.graph.resolve(&id), Resolution::Missing | Resolution::MissingAnchor { .. }) {
                    seen.warn(n, format!("names {id}, which is no document here"));
                }
            }
        }
        if ext.check_paths && !fence.inside() {
            for c in code_re().captures_iter(line) {
                let code = c.get(1).map_or("", |m| m.as_str());
                if let Some(t) = path_token(code, false)
                    && !path_exists(&cfg.spec_home, links, t)
                {
                    seen.warn(n, format!("names {t}, which does not exist"));
                }
            }
        }
    }
}

impl Rule for ExternalCites {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "V3",
            title: "external sources name documents and paths that exist",
            explain: "`[external.<name>] path` names a directory or a file, outside the repository (an agent's memory, personal notes) or in it (an instruction file), that is read but never part of the graph. With --external, the `[[references]]` and spec-path citations of every source are checked. Per source, `ids = true` checks every document id written there (`type:owner/key` of a type this project has, bare or in `[[…]]`), and `check_paths = true` checks every backticked repository path outside code blocks (as A3 reads them: with a slash, no URL, glob or placeholder); a path inside a submodule counts as existing when the commit the gitlink pins holds it, or when that commit cannot be read. Both are off unless set, and run without --external: in a full run over the whole source, in staged and ranged runs over its changed files inside the repository. Files `[generated] manifest` names are skipped. Findings name `<name>:<path below the source>` (the file name for a source that is one file). Fix the note, or restore what it names. Reported as warnings; `[ratchet]` can keep their count from growing.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        let cfg = &ctx.corpus.cfg;
        let links = vcs::gitlinks(&cfg.spec_home);
        let generated = crate::check::limits::generated::load(cfg).unwrap_or_default();
        for (name, ext) in &cfg.raw.external {
            let facts_on = ext.ids || ext.check_paths;
            if !ctx.opts.external && !facts_on {
                continue;
            }
            let base = cfg.expand_path(&ext.path);
            for file in files(cfg, &base) {
                let (path, repo) = shown(cfg, name, &base, &file);
                let skip = match &repo {
                    Some(rel) => generated.holds(rel) || !ctx.scope.covers_path(rel),
                    None => !ctx.scope.is_all() && !ctx.opts.external,
                };
                if skip {
                    continue;
                }
                let Ok(text) = crate::corpus::overlay::read(&file) else { continue };
                let mut seen = Seen { path: &path, done: BTreeSet::new(), out: vec![] };
                if ctx.opts.external {
                    citations(ctx, &text, &mut seen);
                }
                if facts_on {
                    facts(ctx, ext, &links, &text, &mut seen);
                }
                out.extend(seen.out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_read_bare_or_bracketed_but_not_in_urls_or_remote_references() {
        let found = |s: &str| -> Vec<String> {
            id_re().captures_iter(s).map(|c| format!("{}:{}", &c[1], &c[2])).collect::<Vec<_>>()
        };
        assert_eq!(found("see adr:ws/20260101000000."), ["adr:ws/20260101000000"]);
        assert_eq!(found("[[sot:shop/cart#CO-1]] and plan:ws/1"), ["sot:shop/cart#CO-1", "plan:ws/1"]);
        assert!(found("https://example.com/a and org:adr:x/1 and a.adr:x/1").is_empty());
    }
}
