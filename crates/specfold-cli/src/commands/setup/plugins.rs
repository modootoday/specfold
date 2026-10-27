//! `spec plugins`: what is configured, the exact command each runs, and what it provides.

use crate::cli::args::{global::Global, setup::PluginsArgs};
use crate::output::{Code, Output};
use crate::session;
use anyhow::{Result, bail};
use serde_json::json;
use specfold_core::plugins;

pub fn run(g: &Global, a: &PluginsArgs) -> Result<Output> {
    let cfg = session::config(g)?;
    let loaded = plugins::load(&cfg);
    let selected: Vec<_> = match (a.action.as_deref(), &a.name) {
        (None, _) => loaded.iter().collect(),
        (Some("describe" | "test"), Some(n)) => {
            let l: Vec<_> = loaded.iter().filter(|p| &p.def.name == n).collect();
            if l.is_empty() {
                bail!("no plugin named '{n}'");
            }
            l
        }
        (Some(act), _) => bail!("usage: spec plugins [describe|test <name>] (got '{act}')"),
    };
    let mut o = Output::new("plugins");
    let mut failed = false;
    for p in selected {
        let cmd = p.def.cmd.join(" ");
        match &p.description {
            Ok(d) => {
                let rules: Vec<&str> = d.provides.rules.iter().map(|r| r.id.as_str()).collect();
                let text = format!(
                    "{:<16} ok    {cmd}\n    rules: {}  importers: {}  exporters: {}  audits: {}",
                    p.def.name,
                    rules.join(","),
                    d.provides.importers.join(","),
                    d.provides.exporters.join(","),
                    d.provides.audits.join(",")
                );
                o = o.item(json!({"name": p.def.name, "cmd": p.def.cmd, "ok": true, "provides": d.provides}), text);
            }
            Err(e) => {
                failed = true;
                o = o.item(
                    json!({"name": p.def.name, "cmd": p.def.cmd, "ok": false, "error": e.to_string()}),
                    format!("{:<16} error {cmd}\n    {e}", p.def.name),
                );
            }
        }
    }
    if o.items.is_empty() {
        o = o.foot("no [[plugins]] in spec.toml");
    }
    Ok(if failed { o.code(Code::Env) } else { o.next("spec explain x.<plugin>.<rule>") })
}
