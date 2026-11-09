//! B1: files stay within the size budgets `[budgets]` sets. G2: the `[generated] manifest`
//! reads and names sources that exist.

use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::limits::{budgets, generated};
use crate::check::rule::{Rule, RuleInfo};

pub struct SizeBudgets;
pub struct GeneratedManifest;

/// The files B1 measures: in a full run every file the walk reads that a budget names, else the
/// changed or named files a budget names.
fn candidates(ctx: &Ctx, list: &[budgets::Budget]) -> Vec<String> {
    let cfg = &ctx.corpus.cfg;
    let wanted = |p: &str| list.iter().any(|b| b.matches(p));
    if !ctx.scope.is_all() {
        return ctx.scope.paths.iter().filter(|p| wanted(p)).cloned().collect();
    }
    crate::workspace::walk(cfg, &cfg.spec_home)
        .build()
        .flatten()
        .filter(|e| e.file_type().is_some_and(|t| t.is_file()))
        .filter_map(|e| crate::paths::rel_of(&cfg.spec_home, e.path()))
        .filter(|p| wanted(p))
        .collect()
}

/// B1's measures of the files it judges; the error when budgets cannot be read.
fn measures(ctx: &Ctx) -> anyhow::Result<Vec<budgets::Measure>> {
    let list = budgets::load(&ctx.corpus.cfg)?;
    if list.is_empty() {
        return Ok(vec![]);
    }
    let mut out = vec![];
    for p in candidates(ctx, &list) {
        let Some(text) = ctx.current(&p) else { continue };
        out.extend(budgets::measure(&list, &p, text.as_bytes()));
    }
    Ok(out)
}

impl Rule for SizeBudgets {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "B1",
            title: "files stay within their size budgets",
            explain: "`[budgets]` sets a limit in bytes, lines or both per path glob: in spec.toml (`[budgets.paths] \"AGENTS.md\" = { bytes = 12288, lines = 200 }`) or in another file that owns the numbers (`[budgets] from = \"<file>#<key>\"`, a TOML or JSON table of glob to `{ bytes, lines }`, or an array of `{ path | paths, bytes, lines }`). A full run measures every file the walk reads that a glob names; `--staged` and `--changed` measure the changed files; `spec check --rule B1 --path <file> --json` measures one file, and with `--stdin-path <file>` the text on standard input instead of the file on disk. Each limit exceeded is an error naming the size, the limit and where it was set. A source that cannot be read or parsed, a table of the wrong shape, a bad glob, or an entry with neither limit is an error on spec.toml, never a pass. Generated files are measured too: their size is what a reader loads. Shorten the file, or change the limit where it is set. `--json` adds `budgets`: each file judged with `path`, `bytes`, `lines`, `over`, and `limits` (`glob`, `source`, `max_bytes`, `max_lines`, `over_bytes`, `over_lines`).",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        let list = match measures(ctx) {
            Ok(l) => l,
            Err(e) => {
                out.push(Finding::error("B1", "spec.toml", 1, format!("{e:#}; next: spec explain B1")));
                return;
            }
        };
        for m in list {
            for a in &m.limits {
                let at = format!("{} in {}", a.glob, a.source);
                if let (true, Some(max)) = (a.over_bytes, a.max_bytes) {
                    out.push(Finding::error(
                        "B1",
                        &m.path,
                        1,
                        format!("{} bytes, over the budget of {max} ({at})", m.bytes),
                    ));
                }
                if let (true, Some(max)) = (a.over_lines, a.max_lines) {
                    out.push(Finding::error(
                        "B1",
                        &m.path,
                        1,
                        format!("{} lines, over the budget of {max} ({at})", m.lines),
                    ));
                }
            }
        }
    }
}

impl Rule for GeneratedManifest {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "G2",
            title: "the generated-files manifest reads and its sources exist",
            explain: "`[generated] manifest = \"<file>[#<key>]\"` names a manifest another tool writes (JSON when the name ends in `.json` or the text starts with `{`, otherwise TOML; the same holds for `[budgets] from`): an array of entries, or a table whose `outputs` or `files` key holds one, each with a `path` (text after `#` is dropped) and optionally `source` or `sources`. The files it names are generated copies: agent-asset rules (A1-A4, H1) and citation checks (L6, and external checks) skip them, and their sources are checked instead. G2 reports, as errors, a manifest that cannot be read or parsed and a source that does not exist. Regenerate with the tool that owns the manifest, or fix the manifest path in spec.toml.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        let cfg = &ctx.corpus.cfg;
        let Some(pointer) = &cfg.raw.generated.manifest else { return };
        let g = match generated::load(cfg) {
            Ok(g) => g,
            Err(e) => {
                out.push(Finding::error("G2", "spec.toml", 1, format!("{e:#}; next: spec explain G2")));
                return;
            }
        };
        let file = pointer.split('#').next().unwrap_or(pointer);
        for o in &g.outputs {
            for s in o.sources.iter().filter(|s| !cfg.spec_home.join(s).exists()) {
                out.push(Finding::error(
                    "G2",
                    file,
                    1,
                    format!("{} names the source {s}, which does not exist", o.path),
                ));
            }
        }
    }
}
