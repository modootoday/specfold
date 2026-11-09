//! `[generated] manifest`: the files another tool writes and the sources it writes them from.
//! A generated copy is not content-checked (its source is); rule G2 reports a manifest that
//! cannot be read and a source that does not exist.

use crate::config::Config;
use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;

/// One generated file and the sources it is made from (repository paths).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub path: String,
    pub sources: Vec<String>,
}

/// The manifest's outputs; empty without `[generated] manifest`.
#[derive(Debug, Clone, Default)]
pub struct Generated {
    pub outputs: Vec<Output>,
}

impl Generated {
    /// Whether `path` is a generated file or lies inside a generated directory.
    pub fn holds(&self, path: &str) -> bool {
        self.outputs.iter().any(|o| crate::paths::is_under(path, &o.path))
    }
}

/// Reads the manifest. Its value is an array of entries, or a table whose `outputs` or `files`
/// key is one; an entry is a table with a string `path` (anything after `#` is a place inside
/// the file and is dropped) and optionally `source` (a string) or `sources` (strings).
pub fn load(cfg: &Config) -> Result<Generated> {
    let Some(pointer) = &cfg.raw.generated.manifest else { return Ok(Generated::default()) };
    let v = super::source::read(&cfg.spec_home, pointer).context("[generated] manifest")?;
    parse(&v).with_context(|| format!("[generated] manifest = \"{pointer}\""))
}

fn parse(v: &Value) -> Result<Generated> {
    let list = match v {
        Value::Array(a) => a,
        Value::Object(m) => match m.get("outputs").or_else(|| m.get("files")) {
            Some(Value::Array(a)) => a,
            _ => bail!("no array of entries (the value itself, or its outputs or files key)"),
        },
        _ => bail!("no array of entries (the value itself, or its outputs or files key)"),
    };
    let mut outputs: Vec<Output> = vec![];
    for (i, e) in list.iter().enumerate() {
        let at = || format!("entry {}", i + 1);
        let path = e.get("path").and_then(Value::as_str).ok_or_else(|| anyhow!("{} has no string path", at()))?;
        let path = path.split('#').next().unwrap_or(path).trim_end_matches('/').to_string();
        if path.is_empty() {
            bail!("{} has an empty path", at());
        }
        let mut sources = vec![];
        if let Some(s) = e.get("source") {
            sources.push(s.as_str().ok_or_else(|| anyhow!("{}: source is not a string", at()))?.to_string());
        }
        if let Some(s) = e.get("sources") {
            let list = s.as_array().ok_or_else(|| anyhow!("{}: sources is not a list", at()))?;
            for x in list {
                sources.push(x.as_str().ok_or_else(|| anyhow!("{}: a source is not a string", at()))?.to_string());
            }
        }
        match outputs.iter_mut().find(|o| o.path == path) {
            Some(o) => o.sources.extend(sources),
            None => outputs.push(Output { path, sources }),
        }
    }
    for o in &mut outputs {
        o.sources.sort();
        o.sources.dedup();
    }
    Ok(Generated { outputs })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_merge_by_file_and_bad_entries_are_errors() {
        let v = serde_json::json!({"version": 1, "outputs": [
            {"path": "tool.json#a.b", "kind": "json"},
            {"path": "tool.json#a.c", "source": "src/tool.toml"},
            {"path": ".tool/skills/x/", "sources": ["skills/x/SKILL.md"]}
        ]});
        let g = parse(&v).unwrap();
        assert_eq!(g.outputs.len(), 2);
        assert_eq!(g.outputs[0], Output { path: "tool.json".into(), sources: vec!["src/tool.toml".into()] });
        assert!(g.holds(".tool/skills/x/SKILL.md") && g.holds("tool.json") && !g.holds("tool.jsonc"));
        assert!(parse(&serde_json::json!([{"kind": "x"}])).is_err());
        assert!(parse(&serde_json::json!({"items": []})).is_err());
        assert!(parse(&serde_json::json!([{"path": "a", "sources": "b"}])).is_err());
    }
}
