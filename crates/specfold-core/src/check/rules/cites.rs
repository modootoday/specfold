//! L6: documents cited from outside the spec corpus (code comments, READMEs, agent instruction
//! files) still resolve. Old paths keep spreading through text agents read, so a moved or
//! retired target is reported where it is cited, not only inside documents.

use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::check::scope::Range;
use crate::graph::Resolution;
use crate::markdown::body::{path_cite_re, wiki_re};
use crate::workspace::owners::{PathMatch, matcher};
use crate::workspace::vcs;
use ignore::WalkBuilder;
use regex::Regex;
use std::collections::{BTreeSet, HashMap};
use std::sync::OnceLock;

pub struct CitesOutside;

/// Statuses whose documents should no longer be cited as current guidance.
const PAST: &[&str] = &["superseded", "archived", "rejected"];

fn path_token_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?:[A-Za-z0-9_.@-]+/)+[A-Za-z0-9_.@-]+").unwrap())
}

/// `type:owner/key`: outside documents, `[[…]]` is also TOML, shell and array syntax, so only
/// this shape (or a known alias) is read as a reference that must resolve.
fn id_shaped(target: &str) -> bool {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^[a-z][a-z0-9-]*:[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+(#\S+)?$").unwrap()).is_match(target)
}

struct Files {
    include: Vec<PathMatch>,
    exclude: Vec<PathMatch>,
}

impl Files {
    fn wants(&self, ctx: &Ctx, p: &str) -> bool {
        self.include.iter().any(|m| m.is_match(p))
            && !self.exclude.iter().any(|m| m.is_match(p))
            && !ctx.graph.by_path.contains_key(p)
    }
}

/// The files to read and, for staged or ranged runs, the lines each change added.
fn targets(ctx: &Ctx, files: &Files) -> Vec<(String, Option<BTreeSet<usize>>)> {
    let root = &ctx.corpus.cfg.root;
    let added = match &ctx.scope.range {
        Range::All => {
            let walk = WalkBuilder::new(root).hidden(false).build().flatten();
            return walk
                .filter(|e| e.file_type().is_some_and(|t| t.is_file()))
                .filter_map(|e| e.path().strip_prefix(root).ok().map(|p| p.to_string_lossy().replace('\\', "/")))
                .filter(|p| files.wants(ctx, p))
                .map(|p| (p, None))
                .collect();
        }
        Range::Staged => vcs::added_lines(root, None),
        Range::Changed(base) => vcs::added_lines(root, Some(base)),
    };
    let added: HashMap<String, Vec<usize>> = added.unwrap_or_default();
    added
        .into_iter()
        .filter(|(p, _)| files.wants(ctx, p))
        .map(|(p, lines)| (p, Some(lines.into_iter().collect())))
        .collect()
}

fn judge(ctx: &Ctx, path: &str, line: usize, written: &str, target: &str, by_path: bool) -> Option<Finding> {
    let id_of = |idx: usize| ctx.corpus.docs[idx].id.to_string();
    if !by_path {
        use crate::remotes::Remote;
        let problem = match crate::remotes::resolve(&ctx.corpus.cfg, target) {
            Remote::Local => None,
            Remote::Absent { .. } | Remote::Found { .. } => return None,
            Remote::Missing { name, id } => Some(format!("{written} names no document in the remote {name} ({id})")),
            Remote::MissingAnchor { name, id, anchor } => Some(format!("{name}:{id} has no anchor #{anchor}")),
            Remote::Ambiguous { name, ids } => {
                Some(format!("{written} is ambiguous in the remote {name}: {}", ids.join(", ")))
            }
        };
        if let Some(m) = problem {
            return Some(Finding::error("L6", path, line, m));
        }
    }
    match ctx.graph.resolve(target) {
        Resolution::Doc { idx, anchor, via_alias } => {
            let id = match anchor {
                Some(a) => format!("{}#{a}", id_of(idx)),
                None => id_of(idx),
            };
            let status = ctx.corpus.docs[idx].status().unwrap_or_default();
            if via_alias || by_path {
                let how = if via_alias { "an old name" } else { "a path" };
                return Some(
                    Finding::warn("L6", path, line, format!("{written} is {how} for {id}"))
                        .fix_replace(written.to_string(), format!("[[{id}]]")),
                );
            }
            PAST.contains(&status.as_str())
                .then(|| Finding::warn("L6", path, line, format!("{written} cites {id}, which is {status}")))
        }
        Resolution::Ambiguous(list) => {
            let c: Vec<String> = list.into_iter().map(id_of).collect();
            Some(Finding::error("L6", path, line, format!("{written} is ambiguous")).with_candidates(c))
        }
        Resolution::MissingAnchor { idx, anchor } => {
            Some(Finding::error("L6", path, line, format!("{} has no anchor #{anchor}", id_of(idx))))
        }
        Resolution::Missing if by_path || !id_shaped(target) => None,
        Resolution::Missing => Some(Finding::error("L6", path, line, format!("{written} names no document"))),
    }
}

impl Rule for CitesOutside {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "L6",
            title: "citations outside documents still resolve",
            explain: "Files listed in [cites] paths (code, READMEs, agent instructions) are read for [[id]] references, paths into spec directories and old paths from the alias table. A reference that names nothing is an error, and so is `[[<remote>:<id>]]` that names nothing in a configured remote whose directory is present (an absent remote directory is skipped); an old name or a path is reported with the id to write; a citation of a superseded, archived or rejected document is reported. Staged and ranged runs read only the lines the change adds, and lines matching [cites] history are left alone.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        let cfg = &ctx.corpus.cfg.raw.cites;
        if cfg.paths.is_empty() {
            return;
        }
        let files = Files {
            include: cfg.paths.iter().filter_map(|p| matcher(p)).collect(),
            exclude: cfg.exclude.iter().filter_map(|p| matcher(p)).collect(),
        };
        let history: Vec<Regex> = cfg.history.iter().filter_map(|r| Regex::new(r).ok()).collect();
        let spec_paths = path_cite_re(ctx.corpus.cfg.spec_dir());
        for (path, only) in targets(ctx, &files) {
            let Some(text) = ctx.current(&path) else { continue };
            for (n, line) in text.lines().enumerate() {
                let no = n + 1;
                if only.as_ref().is_some_and(|set| !set.contains(&no)) || history.iter().any(|h| h.is_match(line)) {
                    continue;
                }
                let mut seen = BTreeSet::new();
                for c in wiki_re().captures_iter(line) {
                    let target = c[1].trim().to_string();
                    seen.insert(target.clone());
                    out.extend(judge(ctx, &path, no, &c[0], &target, false));
                }
                let tokens = spec_paths.find_iter(line).chain(path_token_re().find_iter(line));
                for m in tokens {
                    let t = m.as_str().trim_end_matches(['.', ',', ':', ';']);
                    if !seen.insert(t.to_string()) {
                        continue;
                    }
                    let known = ctx.graph.by_path.contains_key(t) || ctx.graph.by_alias.contains_key(t);
                    if known {
                        out.extend(judge(ctx, &path, no, t, t, true));
                    }
                }
            }
        }
    }
}
