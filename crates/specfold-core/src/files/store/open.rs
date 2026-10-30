//! Opening the store `[files.store]` configures.

use super::{CommandStore, DirStore, Store};
use crate::config::Config;
use anyhow::{Result, anyhow, bail};

/// The configured store, if any.
pub fn open(cfg: &Config) -> Result<Option<Box<dyn Store>>> {
    let Some(def) = &cfg.raw.files.store else { return Ok(None) };
    match def.kind.as_str() {
        "dir" => {
            let p = def.path.as_deref().ok_or_else(|| anyhow!("files.store.path is required for a dir store"))?;
            Ok(Some(Box::new(DirStore { root: cfg.expand_path(p) })))
        }
        "command" => Ok(Some(Box::new(CommandStore {
            root: cfg.root.clone(),
            put: def.put.clone(),
            get: def.get.clone(),
            exists: def.exists.clone(),
        }))),
        other => bail!("files.store.kind '{other}' is not dir or command"),
    }
}
