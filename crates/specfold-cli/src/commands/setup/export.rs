//! `spec export`: the built-in Markdown tree, or the graph handed to an exporter plugin.

use crate::cli::args::{global::Global, setup::ExportArgs};
use crate::commands::outbound;
use crate::output::{Code, Output};
use crate::session;
use anyhow::{Result, anyhow, bail};
use serde_json::{Value, json};
use specfold_core::export::{self, markdown, payload::Wants};
use specfold_core::plugins::{self, payload, process};

/// Runs the public guards over what is about to leave. Some(output) when they refuse; the hits
/// that let it through are added to `o` either way.
fn guard(
    cfg: &specfold_core::Config,
    a: &ExportArgs,
    blobs: &[(String, Vec<u8>)],
    o: Output,
) -> Result<(Output, bool)> {
    if a.no_guard {
        return Ok((o.foot("public guards skipped (--no-guard)"), false));
    }
    let gate = outbound::gate(cfg, |g| g.public, blobs, &[])?;
    let mut o = o.field("guard", gate.json());
    for l in gate.lines() {
        o = o.head(l);
    }
    if gate.refuses() {
        let n = gate.hits.iter().filter(|(_, f, _)| f.severity == specfold_core::check::Severity::Error).count();
        return Ok((
            o.head(format!("nothing exported: public guard {} found {n} hit(s)", gate.refusing().join(", ")))
                .code(Code::Findings)
                .next("spec guard --range all"),
            true,
        ));
    }
    Ok((o, false))
}

pub fn run(g: &Global, a: &ExportArgs) -> Result<Output> {
    if a.plugin == export::BUILT_IN {
        return built_in(g, a);
    }
    for (set, flag) in [(a.target.is_some(), "--target"), (a.banner, "--banner"), (a.fail_on.is_some(), "--fail-on")] {
        if set {
            bail!("{flag} belongs to the built-in exporter: spec export markdown {flag} …");
        }
    }
    let s = session::open(g)?;
    let loaded = plugins::load(&s.corpus.cfg);
    let p = loaded.iter().find(|p| p.def.name == a.plugin).ok_or_else(|| anyhow!("no plugin named '{}'", a.plugin))?;
    let d = p.description.as_ref().map_err(|e| anyhow!("{e}"))?;
    if d.provides.exporters.is_empty() {
        bail!("plugin {} provides no exporter", a.plugin);
    }
    let format = a.format.clone().unwrap_or_else(|| d.provides.exporters[0].clone());
    let wants = Wants::from(&d.wants);
    // Local documents stay out unless asked for, so private notes never reach a plugin.
    let only = export::payload::visible(&s.corpus, a.local);
    let docs = if wants.any() {
        export::payload::docs(&s.corpus, &s.graph, wants, g.max_bytes, &only)
    } else {
        payload::docs(&s.corpus, Some(&only))
    };
    let params = json!({
        "format": format,
        "out": a.out.as_ref().map(|p| p.display().to_string()),
        "docs": docs,
        "edges": export::payload::edges(&s.corpus, &s.graph, &only),
    });
    let o = Output::new("export").unpaged().field("plugin", a.plugin.clone()).field("format", format.clone());
    // The plugin gets each document's text, so the guard reads the files it comes from.
    let root = &s.corpus.cfg.root;
    let mut blobs = vec![];
    for &i in &only {
        let d = &s.corpus.docs[i];
        for path in std::iter::once(&d.path).chain(&d.chapters) {
            if let Ok(bytes) = std::fs::read(root.join(path)) {
                blobs.push((path.clone(), bytes));
            }
        }
    }
    let (o, refused) = guard(&s.corpus.cfg, a, &blobs, o)?;
    if refused {
        return Ok(o);
    }
    let result: Value = process::call(&s.corpus.cfg, &p.def, "export.run", params)?;
    Ok(o.field("result", result).head(format!("exported with {} ({format})", a.plugin)))
}

fn built_in(g: &Global, a: &ExportArgs) -> Result<Output> {
    let names = markdown::Target::ALL.join(", ");
    let Some(t) = &a.target else { bail!("spec export markdown needs --target ({names})") };
    let target = markdown::Target::parse(t).ok_or_else(|| anyhow!("unknown --target '{t}' (one of {names})"))?;
    let Some(out) = &a.out else { bail!("spec export markdown needs --out <dir>") };
    let strict = match a.fail_on.as_deref() {
        None => false,
        Some("unresolved") => true,
        Some(x) => bail!("unknown --fail-on '{x}' (only: unresolved)"),
    };
    if a.format.is_some() {
        bail!("--format is for plugins; the built-in exporter takes --target");
    }
    let s = session::open(g)?;
    let previous = markdown::check_out(out, &s.corpus.cfg.root)?;
    let opts = markdown::Options { target, local: a.local, banner: a.banner };
    let tree = markdown::build(&s.corpus, &s.graph, &opts);
    let n = tree.unresolved.len();
    let mut o = Output::new("export")
        .unpaged()
        .field("plugin", export::BUILT_IN)
        .field("target", target.name())
        .field("out", out.display().to_string())
        .field("docs", tree.docs)
        .field("files", tree.files.len())
        .field("unresolved", json!(tree.unresolved));
    for u in &tree.unresolved {
        o = o.item(json!(u), format!("{}: {} written as text ({})", u.path, u.raw, u.reason));
    }
    if strict && n > 0 {
        return Ok(o
            .head(format!("nothing written: {n} reference(s) lead nowhere in the tree"))
            .code(Code::Findings)
            .next("spec check --rule L1"));
    }
    let blobs: Vec<(String, Vec<u8>)> = tree.files.iter().map(|(p, t)| (p.clone(), t.as_bytes().to_vec())).collect();
    let (o, refused) = guard(&s.corpus.cfg, a, &blobs, o)?;
    if refused {
        return Ok(o);
    }
    markdown::write(out, &tree, target, &previous)?;
    let mut o = o.head(format!(
        "exported {} document(s) for {} into {} ({} files)",
        tree.docs,
        target.name(),
        out.display(),
        tree.files.len()
    ));
    if n > 0 {
        o = o.foot(format!("{n} reference(s) written as plain text"));
    }
    Ok(o)
}
