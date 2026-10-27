//! Every write is planned first and applied only on request, so a preview and the real run do
//! exactly the same thing.

use anyhow::{Context, Result};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum Change {
    Write {
        path: String,
        content: String,
        created: bool,
    },
    Move {
        from: String,
        to: String,
    },
    /// Copies bytes from a source file (attachments during an import).
    Copy {
        from: String,
        to: String,
    },
    Remove {
        path: String,
    },
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Plan {
    pub changes: Vec<Change>,
}

impl Plan {
    pub fn write(&mut self, path: impl Into<String>, content: String, created: bool) {
        let path = path.into();
        self.changes.retain(|c| !matches!(c, Change::Write { path: p, .. } if *p == path));
        self.changes.push(Change::Write { path, content, created });
    }

    pub fn mv(&mut self, from: impl Into<String>, to: impl Into<String>) {
        self.changes.push(Change::Move { from: from.into(), to: to.into() });
    }

    pub fn copy(&mut self, from: impl Into<String>, to: impl Into<String>) {
        self.changes.push(Change::Copy { from: from.into(), to: to.into() });
    }

    pub fn remove(&mut self, path: impl Into<String>) {
        self.changes.push(Change::Remove { path: path.into() });
    }

    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Paths touched, for reports.
    pub fn paths(&self) -> Vec<String> {
        self.changes
            .iter()
            .map(|c| match c {
                Change::Write { path, .. } | Change::Remove { path } => path.clone(),
                Change::Move { from, to } | Change::Copy { from, to } => format!("{from} -> {to}"),
            })
            .collect()
    }

    /// Moves and copies first (so writes land at the new paths), then writes, then removals.
    pub fn apply(&self, root: &Path) -> Result<()> {
        let ensure_parent = |p: &Path| -> Result<()> {
            if let Some(d) = p.parent() {
                std::fs::create_dir_all(d)?;
            }
            Ok(())
        };
        for c in &self.changes {
            match c {
                Change::Move { from, to } => {
                    let dst = root.join(to);
                    ensure_parent(&dst)?;
                    std::fs::rename(root.join(from), &dst).with_context(|| format!("cannot move {from} to {to}"))?;
                }
                Change::Copy { from, to } => {
                    let dst = root.join(to);
                    ensure_parent(&dst)?;
                    std::fs::copy(root.join(from), &dst).with_context(|| format!("cannot copy {from} to {to}"))?;
                }
                _ => {}
            }
        }
        for c in &self.changes {
            if let Change::Write { path, content, .. } = c {
                let dst = root.join(path);
                ensure_parent(&dst)?;
                std::fs::write(&dst, content).with_context(|| format!("cannot write {path}"))?;
            }
        }
        for c in &self.changes {
            if let Change::Remove { path } = c {
                let p = root.join(path);
                if p.is_dir() {
                    std::fs::remove_dir_all(&p).with_context(|| format!("cannot remove {path}"))?;
                } else if p.exists() {
                    std::fs::remove_file(&p).with_context(|| format!("cannot remove {path}"))?;
                }
            }
        }
        Ok(())
    }
}
