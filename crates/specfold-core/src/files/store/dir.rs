//! A store that is a plain directory: a shared drive, a synced folder, or a test fixture.

use super::types::Store;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub struct DirStore {
    pub root: PathBuf,
}

impl DirStore {
    fn object(&self, key: &str) -> PathBuf {
        self.root.join(&key[..2.min(key.len())]).join(key)
    }
}

impl Store for DirStore {
    fn put(&self, src: &Path, key: &str) -> Result<()> {
        let dst = self.object(key);
        if dst.is_file() {
            return Ok(());
        }
        std::fs::create_dir_all(dst.parent().unwrap_or(&self.root))?;
        std::fs::copy(src, &dst).with_context(|| format!("cannot copy into {}", dst.display()))?;
        Ok(())
    }

    fn get(&self, key: &str, dst: &Path) -> Result<()> {
        if let Some(p) = dst.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::copy(self.object(key), dst)
            .with_context(|| format!("object {key} is not in {}", self.root.display()))?;
        Ok(())
    }

    fn exists(&self, key: &str) -> Result<Option<bool>> {
        Ok(Some(self.object(key).is_file()))
    }
}
