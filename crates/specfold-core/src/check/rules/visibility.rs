//! V rules: private material never reaches a public path, and committed documents never depend on
//! local ones.

use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::graph::Resolution;
use crate::markdown::body;
use crate::model::Tier;
use crate::workspace::exposure::{self, Exposed};
use ignore::WalkBuilder;

pub struct NotInPublic;
pub struct NoLocalCites;
pub struct ExternalCites;

fn is_spec_file(name: &str, types: &[&str]) -> bool {
    types.iter().any(|t| name.ends_with(&format!(".{t}.md")))
}

impl Rule for NotInPublic {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "V1",
            title: "no spec documents under public paths",
            explain: "Published paths hold no spec document: those listed in [public] paths, public submodules in [public] submodules_manifest (which must match .gitmodules), and what [public] detect finds (a docs/ site, a static-site generator, an npm package or crate that would ship the spec directory). Material about them is stored with their private companion ([public.companion]) or at the root. `spec mv <id>` moves a document to where it belongs.",
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
            let base = crate::placement::literal_base(p);
            let dir = cfg.root.join(&base);
            if !dir.is_dir() {
                continue;
            }
            for e in WalkBuilder::new(&dir).hidden(false).require_git(false).build().flatten() {
                let Some(rel) = e.path().strip_prefix(&cfg.root).ok().map(|r| r.to_string_lossy().replace('\\', "/"))
                else {
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

impl Rule for ExternalCites {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "V3",
            title: "external notes cite documents that exist",
            explain: "External roots (an agent's memory, personal notes) are read but never checked into the graph. With --external, their [[references]] and spec-path citations are checked so stale notes can be fixed.",
            on_commit: false,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        if !ctx.opts.external {
            return;
        }
        let cfg = &ctx.corpus.cfg;
        let re = body::path_cite_re(cfg.spec_dir());
        for (name, ext) in &cfg.raw.external {
            let dir = cfg.expand_path(&ext.path);
            for e in WalkBuilder::new(&dir).standard_filters(false).build().flatten() {
                if !e.file_type().is_some_and(|t| t.is_file()) || e.path().extension().is_none_or(|x| x != "md") {
                    continue;
                }
                let Ok(text) = crate::corpus::overlay::read(e.path()) else { continue };
                let shown = format!("{name}:{}", e.path().strip_prefix(&dir).unwrap_or(e.path()).display());
                let scan = body::scan(&text, 1, &re);
                for l in scan.links.iter().filter(|l| l.text.contains(':') || l.text.contains('/')) {
                    if matches!(ctx.graph.resolve(&l.text), Resolution::Missing | Resolution::MissingAnchor { .. }) {
                        out.push(Finding::warn("V3", &shown, l.line, format!("[[{}]] names no document", l.text)));
                    }
                }
                for c in &scan.path_cites {
                    match ctx.graph.resolve(&c.text) {
                        Resolution::Doc { idx, .. } => out.push(
                            Finding::warn("V3", &shown, c.line, format!("cites the path {}", c.text))
                                .fix_replace(c.text.clone(), format!("[[{}]]", ctx.corpus.docs[idx].id)),
                        ),
                        _ => out.push(Finding::warn(
                            "V3",
                            &shown,
                            c.line,
                            format!("cites {} which does not exist", c.text),
                        )),
                    }
                }
            }
        }
    }
}
