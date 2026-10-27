//! The configured plugins and what each one says it provides.

use super::process;
use super::protocol::{Description, PROTOCOL};
use crate::config::{Config, PluginDef};
use anyhow::{Result, bail};

pub struct Loaded {
    pub def: PluginDef,
    pub description: Result<Description>,
}

pub fn describe(cfg: &Config, p: &PluginDef) -> Result<Description> {
    let d: Description = process::call(cfg, p, "describe", serde_json::Value::Null)?;
    if d.protocol != PROTOCOL {
        bail!("plugin {} speaks '{}', this specfold speaks {PROTOCOL}", p.name, d.protocol);
    }
    Ok(d)
}

/// Asks every configured plugin to describe itself. Failures are kept, not fatal.
pub fn load(cfg: &Config) -> Vec<Loaded> {
    cfg.raw.plugins.iter().map(|p| Loaded { def: p.clone(), description: describe(cfg, p) }).collect()
}
