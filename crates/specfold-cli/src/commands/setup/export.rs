//! `spec export`: hands the graph to an exporter plugin.

use crate::cli::args::{global::Global, setup::ExportArgs};
use crate::output::Output;
use crate::session;
use anyhow::{Result, anyhow, bail};
use serde_json::{Value, json};
use specfold_core::plugins::{self, payload, process};

pub fn run(g: &Global, a: &ExportArgs) -> Result<Output> {
    let s = session::open(g)?;
    let loaded = plugins::load(&s.corpus.cfg);
    let p = loaded.iter().find(|p| p.def.name == a.plugin).ok_or_else(|| anyhow!("no plugin named '{}'", a.plugin))?;
    let d = p.description.as_ref().map_err(|e| anyhow!("{e}"))?;
    if d.provides.exporters.is_empty() {
        bail!("plugin {} provides no exporter", a.plugin);
    }
    let format = a.format.clone().unwrap_or_else(|| d.provides.exporters[0].clone());
    let params = json!({
        "format": format,
        "out": a.out.as_ref().map(|p| p.display().to_string()),
        "docs": payload::docs(&s.corpus, None),
        "edges": payload::edges(&s.corpus, &s.graph),
    });
    let result: Value = process::call(&s.corpus.cfg, &p.def, "export.run", params)?;
    Ok(Output::new("export")
        .unpaged()
        .field("plugin", a.plugin.clone())
        .field("format", format.clone())
        .field("result", result)
        .head(format!("exported with {} ({format})", a.plugin)))
}
