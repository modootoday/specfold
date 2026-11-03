//! The store interface. Objects are content-addressed: the key is the file's SHA-256.

use anyhow::Result;
use std::path::Path;

pub trait Store {
    fn put(&self, src: &Path, key: &str) -> Result<()>;
    fn get(&self, key: &str, dst: &Path) -> Result<()>;
    /// `None` when the store cannot tell without side effects.
    fn exists(&self, key: &str) -> Result<Option<bool>>;
}
