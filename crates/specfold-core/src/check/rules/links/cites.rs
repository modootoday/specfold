//! L6: documents cited from outside the spec corpus (code comments, READMEs, agent instruction
//! files) still resolve. Old paths keep spreading through text agents read, so a moved or
//! retired target is reported where it is cited, not only inside documents.

use super::cites_scan::{self, Patterns, Source};
use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::check::scope::Range;
use crate::corpus::trace;
use crate::graph::Resolution;
use crate::markdown::body::path_cite_re;
use crate::workspace::owners::{PathMatch, matcher};
use crate::workspace::vcs;
use regex::Regex;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::OnceLock;

pub struct CitesOutside;

/// Statuses whose documents should no longer be cited as current guidance.
const PAST: &[&str] = &["superseded", "archived", "rejected"];

fn path_token_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?:[A-Za-z0-9_.@-]+/)+[A-Za-z0-9_.@-]+").expect("static regex is valid"))
}

/// `type:owner/key`: outside documents, `[[…]]` is also TOML, shell and array syntax, so only
/// this shape (or a known alias) is read as a reference that must resolve.
fn id_shaped(target: &str) -> bool {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"^[a-z][a-z0-9-]*:[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+(#\S+)?$").expect("static regex is valid")
    })
    .is_match(target)
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
            && !ctx.corpus.cfg.raw.owners.in_nested(p)
    }
}

/// The files to read and, for staged or ranged runs, the lines each change added.
fn targets(ctx: &Ctx, files: &Files) -> Vec<(String, Option<BTreeSet<usize>>)> {
    let root = &ctx.corpus.cfg.root;
    let added = match &ctx.scope.range {
        Range::All => {
            let walk = crate::workspace::walk(&ctx.corpus.cfg, root).build().flatten();
            return walk
                .filter(|e| e.file_type().is_some_and(|t| t.is_file()))
                .filter_map(|e| crate::paths::rel_of(root, e.path()))
                .filter(|p| files.wants(ctx, p))
                .map(|p| (p, None))
                .collect();
        }
        Range::Staged => vcs::added_lines(root, None),
        Range::Changed(base) => vcs::added_lines(root, Some(base)),
        Range::Commit(sha) => vcs::added_lines(root, Some(&vcs::commit_diff(root, sha))),
    };
    let added: HashMap<String, Vec<usize>> = added.unwrap_or_default();
    added
        .into_iter()
        .filter(|(p, _)| files.wants(ctx, p))
        .map(|(p, lines)| (p, Some(lines.into_iter().collect())))
        .collect()
}

/// Remote lookups already made in this run: each touches the filesystem, and a target is often
/// cited many times (B2).
type Memo = HashMap<String, crate::remotes::Remote>;

fn judge(ctx: &Ctx, memo: &mut Memo, at: (&str, usize), written: &str, target: &str, by_path: bool) -> Option<Finding> {
    let (path, line) = at;
    let id_of = |idx: usize| ctx.corpus.docs[idx].id.to_string();
    if !by_path {
        use crate::remotes::Remote;
        let found =
            memo.entry(target.to_string()).or_insert_with(|| crate::remotes::resolve(&ctx.corpus.cfg, target)).clone();
        let problem = match found {
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
            explain: "Files listed in [cites] paths (code, READMEs, agent instructions) are read for [[id]] references, paths into spec directories and old paths from the alias table; a bash test such as `[[ -f x ]]` or `[[ $a == b ]]` is not a reference. A reference that names nothing is an error, and so is `[[<remote>:<id>]]` that names nothing in a configured remote whose directory is present (an absent remote directory is skipped); an old name or a path is reported with the id to write; a citation of a superseded, archived or rejected document is reported. Staged and ranged runs read only the lines the change adds, and lines matching [cites] history are left alone. Files inside a nested project ([owners] nested) are its own to check.",
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
        let start = std::time::Instant::now();
        let list = targets(ctx, &files);
        if trace::enabled() {
            eprintln!("specfold:   L6 list      {:>7.1} ms, {} files", ms(start), list.len());
        }
        let pats = Patterns { spec_paths: &spec_paths, token: path_token_re(), history: &history };
        let known = |t: &str| ctx.graph.by_path.contains_key(t) || ctx.graph.by_alias.contains_key(t);
        let names: HashSet<u64> =
            ctx.graph.by_path.keys().chain(ctx.graph.by_alias.keys()).map(|k| cites_scan::token_hash(k)).collect();
        let src = match ctx.scope.range {
            Range::Staged => Source::Staged(&ctx.scope.in_submodules),
            _ => Source::Disk,
        };
        let extracts = cites_scan::run(&ctx.corpus.cfg, &list, src, &pats, &known, &names);
        let start = std::time::Instant::now();
        let mut memo = Memo::new();
        for ((path, _), ex) in list.iter().zip(extracts) {
            for (no, written, target) in &ex.wiki {
                out.extend(judge(ctx, &mut memo, (path, *no), written, target, false));
            }
            for (no, t) in ex.tokens.iter().filter(|(_, t)| known(t)) {
                out.extend(judge(ctx, &mut memo, (path, *no), t, t, true));
            }
        }
        if trace::enabled() {
            eprintln!("specfold:   L6 judge     {:>7.1} ms, {} remote targets", ms(start), memo.len());
        }
    }
}

fn ms(start: std::time::Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}
