//! V rules: private material never reaches a public path, and committed documents never depend on
//! local ones (V3, external sources, lives in its own module).

use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::corpus::graph::Resolution;
use crate::model::Tier;
use crate::workspace::exposure::{self, Exposed};
use crate::workspace::walk;

pub struct NotInPublic;
pub struct NoLocalCites;

fn is_spec_file(name: &str, types: &[&str]) -> bool {
    types.iter().any(|t| name.ends_with(&format!(".{t}.md")))
}

impl Rule for NotInPublic {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "V1",
            title: "no spec documents under public paths",
            explain: "Published paths hold no spec document: those listed in [public] paths, public submodules in [public] submodules_manifest (which must match .gitmodules; `spec sync visibility --write` measures it again), and what [public] detect finds (a docs/ site, a static-site generator, an npm package or crate whose `files` or `include` would ship the spec directory or a type's own directory, `[types.<name>] dir`). A nested project ([owners] nested) is left to its own checks. Material about them is stored with their private companion ([public.companion]) or at the root. `spec mv <id>` moves a document to where it belongs.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        let cfg = &ctx.corpus.cfg;
        let types = cfg.registry.names();
        let report = exposure::scan(cfg);
        if ctx.scope.is_all() {
            for (path, msg) in &report.problems {
                out.push(Finding::error("V1", path, 1, msg.clone()));
            }
        }
        for Exposed { path: p, reason } in &report.exposed {
            let base = crate::workspace::placement::literal_base(p);
            let dir = cfg.spec_home.join(&base);
            if !dir.is_dir() {
                continue;
            }
            for e in walk(cfg, &dir).require_git(false).build().flatten() {
                let Some(rel) = crate::paths::rel_of(&cfg.spec_home, e.path()) else {
                    continue;
                };
                let name = e.file_name().to_string_lossy();
                let is_spec_dir = e.file_type().is_some_and(|t| t.is_dir()) && name == cfg.spec_dir();
                let is_doc = e.file_type().is_some_and(|t| t.is_file()) && is_spec_file(&name, &types);
                let in_scope = if is_spec_dir { ctx.scope.is_all() } else { is_doc && ctx.scope.covers_path(&rel) };
                if in_scope {
                    out.push(Finding::error(
                        "V1",
                        &rel,
                        1,
                        format!("{rel} is inside the published path {p} ({reason})"),
                    ));
                }
            }
        }
    }
}

impl Rule for NoLocalCites {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "V2",
            title: "committed documents do not cite local ones",
            explain: "The local directory is ignored by git, so a committed reference to it breaks for everyone else. Commit the cited document first, or drop the reference.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if d.tier != Tier::Committed || !ctx.scope.covers_links(i) {
                continue;
            }
            for l in &d.links {
                if let Resolution::Doc { idx, .. } = ctx.graph.resolve(&l.item)
                    && ctx.corpus.docs[idx].tier == Tier::Local
                {
                    out.push(Finding::error(
                        "V2",
                        &l.path,
                        l.line,
                        format!("cites the local document {}", ctx.corpus.docs[idx].id),
                    ));
                }
            }
        }
    }
}
