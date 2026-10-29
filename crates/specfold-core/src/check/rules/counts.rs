//! N1: a number wrapped in a `spec:count` marker equals what its source counts. Sources are
//! spec's own queries only (`find` filters, table rows), so checking never runs project code.

use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::markdown::counts::{self, Marker};
use crate::search::{self, Query};
use crate::workspace::owners::matcher;
use ignore::WalkBuilder;

pub struct CountsMatch;

/// What a marker's source counts in `text`, or why it cannot be counted.
fn count(ctx: &Ctx, text: &str, source: &str) -> Result<usize, String> {
    let words = split(source)?;
    match words.first().map(String::as_str) {
        Some("find") => {
            let mut q = Query::default();
            for w in &words[1..] {
                let (k, v) = w.split_once('=').ok_or_else(|| format!("expected key=value, got '{w}'"))?;
                let vals = v.split(',').map(str::to_string);
                match k {
                    "type" => q.types.extend(vals),
                    "owner" => q.owners.extend(vals),
                    "status" => q.statuses.extend(vals),
                    "tag" => q.tags.extend(vals),
                    _ => return Err(format!("unknown find filter '{k}' (type, owner, status, tag)")),
                }
            }
            // A status filter names the lifecycle wanted, so it reaches past current documents.
            q.all = !q.statuses.is_empty();
            Ok(search::find(ctx.corpus, &q).len())
        }
        Some(w) if w.starts_with("rows=") => {
            if words.len() > 1 {
                return Err("rows=<section> takes nothing else".into());
            }
            let sec = &w["rows=".len()..];
            counts::table_rows(text, sec).ok_or_else(|| format!("no section '{sec}' in this file"))
        }
        _ => Err("source must be `find <filters>` or `rows=<section>`".into()),
    }
}

/// Splits on spaces, keeping double-quoted parts together (`rows="Open items"`).
fn split(s: &str) -> Result<Vec<String>, String> {
    let (mut out, mut cur, mut quoted) = (vec![], String::new(), false);
    for c in s.chars() {
        match c {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if quoted {
        return Err("unclosed quote".into());
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    Ok(out)
}

fn check_marker(ctx: &Ctx, path: &str, text: &str, m: &Marker) -> Option<Finding> {
    let n = match count(ctx, text, &m.source) {
        Ok(n) => n,
        Err(why) => return Some(Finding::error("N1", path, m.line, format!("spec:count '{}': {why}", m.source))),
    };
    if m.value.parse::<usize>().ok() == Some(n) {
        return None;
    }
    let shown = if m.value.is_empty() { "nothing".to_string() } else { format!("'{}'", m.value) };
    Some(
        Finding::warn("N1", path, m.line, format!("spec:count {} says {shown} but counts {n}", m.source))
            .fix_replace(m.raw.clone(), counts::rewrite(m, n)),
    )
}

/// Markdown files named by `[check] count_paths`, limited to the run's paths when it has a range.
fn extra_files(ctx: &Ctx) -> Vec<String> {
    let cfg = &ctx.corpus.cfg;
    let pats: Vec<_> = cfg.raw.check.count_paths.iter().filter_map(|p| matcher(p)).collect();
    if pats.is_empty() {
        return vec![];
    }
    let hit = |p: &str| p.ends_with(".md") && pats.iter().any(|m| m.is_match(p));
    if !ctx.scope.is_all() {
        return ctx.scope.paths.iter().filter(|p| hit(p)).cloned().collect();
    }
    WalkBuilder::new(&cfg.root)
        .hidden(false)
        .build()
        .flatten()
        .filter_map(|e| e.path().strip_prefix(&cfg.root).ok().map(|p| p.to_string_lossy().replace('\\', "/")))
        .filter(|p| hit(p))
        .collect()
}

impl Rule for CountsMatch {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "N1",
            title: "counts in spec:count markers are current",
            explain: "A number in prose drifts as documents come and go. Wrap it so spec knows where it comes from: `<!-- spec:count find type=adr status=active -->12<!-- /spec:count -->` counts documents like `spec find` (filters type, owner, status, tag; comma for several values; without status only current documents count), and `<!-- spec:count rows=\"Open items\" -->3<!-- /spec:count -->` counts the body rows of the tables in that section of the same file. Open and close on one line; markers cannot nest and are ignored inside code. A wrong number is a warning that `spec fix --write` rewrites; an unreadable marker is an error to fix by hand. Files outside documents (README.md, docs/**/*.md) are checked only when listed in `[check] count_paths`.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        let mut files = vec![];
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if ctx.scope.covers(i) {
                files.extend(std::iter::once(d.path.clone()).chain(d.chapters.iter().cloned()));
            }
        }
        files.extend(extra_files(ctx));
        files.sort();
        files.dedup();
        for path in &files {
            let Some(text) = ctx.current(path) else { continue };
            if !text.contains("spec:count") {
                continue;
            }
            let (ok, bad) = counts::scan(&text);
            out.extend(bad.into_iter().map(|b| Finding::error("N1", path, b.line, b.message)));
            out.extend(ok.iter().filter_map(|m| check_marker(ctx, path, &text, m)));
        }
    }
}
