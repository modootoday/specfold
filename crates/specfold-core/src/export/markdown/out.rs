//! Checking the `--out` folder of a Markdown export before anything is written.

use super::MANIFEST;
use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Checks `out` can take an export (missing, empty or an earlier export, outside the repository
/// and `.spec`/`.git`) and returns the files the earlier export listed. `abs` is `out` made
/// absolute by the caller; `out` is how messages name it.
pub fn check_out(out: &Path, abs: &Path, root: &Path) -> Result<Vec<String>> {
    // C221: the nearest existing folder resolved, so a symbolic link cannot hide a spec or git
    // directory; the missing rest is appended as written.
    let real = crate::paths::real_with_missing_tail(abs).unwrap_or_else(|| abs.to_path_buf());
    let spec_or_git =
        |p: &Path| p.components().any(|c| matches!(c.as_os_str().to_str(), Some(".spec" | ".spec.local" | ".git")));
    if spec_or_git(out) || spec_or_git(&real) {
        bail!("refusing to export into {}: it is inside a spec or git directory", out.display());
    }
    let root_real = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    if root_real.starts_with(&real) {
        bail!("refusing to export into {}: it contains the repository", out.display());
    }
    // C259: a dangling link reads as missing but cannot be created as a folder; refuse in preview too.
    if !abs.exists() && abs.symlink_metadata().is_ok() {
        let to = std::fs::read_link(abs).map(|t| t.display().to_string()).unwrap_or_default();
        let target = out.parent().map_or_else(|| PathBuf::from(&to), |p| p.join(&to));
        bail!(
            "{} is a symbolic link to {to}, which does not exist; create that folder or give another --out; next: mkdir -p {}",
            out.display(),
            target.display()
        );
    }
    if !abs.exists() {
        return Ok(vec![]);
    }
    if !abs.is_dir() {
        bail!("{} is not a directory", out.display());
    }
    let canon = abs.canonicalize()?;
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    if root.starts_with(&canon) {
        bail!("refusing to export into {}: it contains the repository", out.display());
    }
    let m = out.join(MANIFEST);
    if abs.join(MANIFEST).is_file() {
        let v: Value = serde_json::from_str(&std::fs::read_to_string(abs.join(MANIFEST))?)
            .with_context(|| format!("reading {}", m.display()))?;
        return Ok(v["files"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|f| f.as_str().map(str::to_string))
            .collect());
    }
    if std::fs::read_dir(abs)?.next().is_some() {
        bail!(
            "refusing to export into {}: it is not empty and holds no {MANIFEST} from an earlier export",
            out.display()
        );
    }
    Ok(vec![])
}
