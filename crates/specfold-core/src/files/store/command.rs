//! A store reached through a command the user already trusts (rclone, aws, gsutil, wrangler …).
//! specfold never handles credentials: the command reads them from its own configuration.

use super::types::Store;
use anyhow::{Result, anyhow, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct CommandStore {
    /// Commands run from the repository root, so a relative program (a local tool wrapper)
    /// resolves the same wherever specfold was started.
    pub root: PathBuf,
    pub put: Option<Vec<String>>,
    pub get: Option<Vec<String>>,
    pub exists: Option<Vec<String>>,
}

impl CommandStore {
    fn run(&self, template: &[String], key: &str, src: &str, dst: &str) -> Result<std::process::Output> {
        let args: Vec<String> =
            template.iter().map(|a| a.replace("{key}", key).replace("{src}", src).replace("{dst}", dst)).collect();
        let (prog, rest) = args.split_first().ok_or_else(|| anyhow!("empty store command"))?;
        let local = self.root.join(prog);
        let program = if prog.contains('/') && local.exists() { local } else { PathBuf::from(prog) };
        Command::new(&program)
            .args(rest)
            .current_dir(&self.root)
            .output()
            .map_err(|e| anyhow!("cannot run {prog}: {e}"))
    }
}

impl Store for CommandStore {
    fn put(&self, src: &Path, key: &str) -> Result<()> {
        let t = self.put.as_ref().ok_or_else(|| anyhow!("files.store.put is not configured"))?;
        let out = self.run(t, key, &src.to_string_lossy(), "")?;
        if !out.status.success() {
            bail!("store put failed: {}", String::from_utf8_lossy(&out.stderr).trim());
        }
        Ok(())
    }

    fn get(&self, key: &str, dst: &Path) -> Result<()> {
        let t = self.get.as_ref().ok_or_else(|| anyhow!("files.store.get is not configured"))?;
        if let Some(p) = dst.parent() {
            std::fs::create_dir_all(p)?;
        }
        let out = self.run(t, key, "", &dst.to_string_lossy())?;
        if !out.status.success() {
            bail!("store get failed: {}", String::from_utf8_lossy(&out.stderr).trim());
        }
        Ok(())
    }

    fn exists(&self, key: &str) -> Result<Option<bool>> {
        let Some(t) = &self.exists else { return Ok(None) };
        Ok(Some(self.run(t, key, "", "")?.status.success()))
    }
}
