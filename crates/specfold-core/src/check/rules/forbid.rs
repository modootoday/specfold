//! Z1: layouts and identifiers a project has retired do not come back.

use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::workspace::owners::matcher;
use ignore::WalkBuilder;
use regex::Regex;

pub struct Forbidden;

impl Rule for Forbidden {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "Z1",
            title: "retired layouts and identifiers stay retired",
            explain: "[forbid] paths lists files and directories that must not exist (an old docs folder, a hand-kept index); [forbid] patterns lists text that must not appear in documents (an old id scheme). Both are project settings; this rule only enforces them.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        let cfg = &ctx.corpus.cfg;
        let paths: Vec<_> = cfg.raw.forbid.paths.iter().filter_map(|p| matcher(p).map(|m| (p.clone(), m))).collect();
        if !paths.is_empty() {
            let candidates: Vec<String> = if ctx.scope.is_all() {
                WalkBuilder::new(&cfg.root)
                    .hidden(false)
                    .require_git(false)
                    .build()
                    .flatten()
                    .filter_map(|e| {
                        e.path().strip_prefix(&cfg.root).ok().map(|p| p.to_string_lossy().replace('\\', "/"))
                    })
                    .filter(|p| !p.is_empty())
                    .collect()
            } else {
                ctx.scope.paths.iter().cloned().collect()
            };
            for p in candidates {
                if let Some((pat, _)) = paths.iter().find(|(_, m)| m.is_match(&p)) {
                    out.push(Finding::error("Z1", &p, 1, format!("matches the forbidden path {pat}")));
                }
            }
        }
        let patterns: Vec<Regex> = cfg.raw.forbid.patterns.iter().filter_map(|p| Regex::new(p).ok()).collect();
        if patterns.is_empty() {
            return;
        }
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers(i) {
                continue;
            }
            for file in std::iter::once(&d.path).chain(&d.chapters) {
                let Ok(text) = crate::corpus::overlay::read(&cfg.root.join(file)) else { continue };
                for (n, line) in text.lines().enumerate() {
                    if let Some(re) = patterns.iter().find(|r| r.is_match(line)) {
                        out.push(Finding::error(
                            "Z1",
                            file,
                            n + 1,
                            format!("contains the forbidden pattern {}", re.as_str()),
                        ));
                    }
                }
            }
        }
    }
}
