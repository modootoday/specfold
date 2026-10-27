//! Plugin rules: run each plugin that provides rules and turn its answer into findings.

use super::protocol::FindingsResponse;
use super::registry::Loaded;
use super::{payload, process};
use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::scope::Range;
use serde_json::json;

fn rule_id(plugin: &str, rule: &str) -> String {
    format!("x.{plugin}.{rule}")
}

pub fn run(ctx: &Ctx, plugins: &[Loaded], out: &mut Vec<Finding>) {
    let staged = matches!(ctx.scope.range, Range::Staged);
    for p in plugins {
        let d = match &p.description {
            Ok(d) => d,
            Err(e) => {
                out.push(Finding::error(rule_id(&p.def.name, "describe"), "spec.toml", 1, e.to_string()));
                continue;
            }
        };
        let rules: Vec<_> = d.provides.rules.iter().filter(|r| !staged || r.on_commit).collect();
        if rules.is_empty() {
            continue;
        }
        let scope_docs: Option<Vec<usize>> =
            (!ctx.scope.is_all()).then(|| ctx.scope.with_neighbours.iter().copied().collect());
        let params = json!({
            "scope": match ctx.scope.range { Range::All => "all", Range::Staged => "staged", Range::Changed(_) => "changed" },
            "rules": rules.iter().map(|r| &r.id).collect::<Vec<_>>(),
            "docs": payload::docs(ctx.corpus, scope_docs.as_deref()),
            "edges": payload::edges(ctx.corpus, ctx.graph),
            "changed_paths": ctx.scope.paths,
        });
        match process::call::<FindingsResponse>(&ctx.corpus.cfg, &p.def, "rule.run", params) {
            Ok(resp) => {
                for f in resp.findings {
                    out.push(Finding {
                        rule: rule_id(&p.def.name, &f.rule),
                        severity: f.severity,
                        path: f.path,
                        line: f.line,
                        message: f.message,
                        fix: f.fix,
                        candidates: f.candidates,
                    });
                }
            }
            Err(e) => out.push(Finding::error(rule_id(&p.def.name, "run"), "spec.toml", 1, e.to_string())),
        }
    }
}
