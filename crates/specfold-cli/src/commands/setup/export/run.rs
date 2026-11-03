//! `spec export`: the built-in Markdown tree, or the graph handed to an exporter plugin.

use super::gate::{guard, source_blobs, unscanned};
use super::markdown::built_in;
use crate::cli::args::{global::Global, setup::ExportArgs};
use crate::output::Output;
use crate::session;
use anyhow::{Result, anyhow, bail};
use serde_json::{Value, json};
use specfold_core::export::{self, payload::Wants};
use specfold_core::plugins::{self, payload, process};

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
    if wants.any() {
        export::payload::readable(&s.corpus, &only)?;
    }
    let docs = if wants.any() {
        export::payload::docs(&s.corpus, &s.graph, wants, g.max_bytes, &only)
    } else {
        payload::docs(&s.corpus, Some(&only))
    };
    let params = json!({
        "format": format,
        "out": a.out.as_ref().map(|p| s.loc.arg_path(p).display().to_string()),
        "docs": docs,
        "edges": export::payload::edges(&s.corpus, &s.graph, &only),
    });
    let o = Output::new("export").unpaged().field("plugin", a.plugin.clone()).field("format", format.clone());
    // The plugin gets each document's text, so the guard reads the files it comes from.
    let blobs = source_blobs(&s.corpus, &only);
    let (o, refused) = guard(&s.corpus.cfg, a, &blobs, &blobs, o)?;
    if refused {
        return Ok(o);
    }
    let o = match (a.no_guard, unscanned(&s.corpus, &only)) {
        (false, Some(note)) => o.field("not_scanned", note.clone()).foot(note),
        _ => o,
    };
    // C114: a preview never runs the plugin, since the plugin is what sends.
    if a.dry_run {
        let n = docs.as_array().map_or(0, Vec::len);
        return Ok(o
            .field("written", false)
            .field("docs", n)
            .head(format!("would send {n} document(s) to {} ({format}): nothing was sent", a.plugin))
            .next(format!("spec export {} --format {format}", a.plugin)));
    }
    let result: Value = process::call(&s.corpus.cfg, &p.def, "export.run", params)?;
    Ok(o.field("result", result).head(format!("exported with {} ({format})", a.plugin)))
}
