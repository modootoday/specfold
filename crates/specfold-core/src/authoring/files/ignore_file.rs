//! The `.gitignore` specfold keeps inside a bundle, so files held in the store are never committed
//! even after `spec fetch` puts them back on disk.

use anyhow::Result;
use std::path::Path;

pub const NAME: &str = ".gitignore";
const HEADER: &str = "# Managed by specfold: these attachments live in the store.";

pub fn add(bundle: &Path, paths: &[String]) -> Result<()> {
    let file = bundle.join(NAME);
    let mut lines: Vec<String> =
        std::fs::read_to_string(&file).unwrap_or_default().lines().map(str::to_string).collect();
    if lines.is_empty() {
        lines.push(HEADER.into());
    }
    for p in paths {
        let entry = format!("/{p}");
        if !lines.contains(&entry) {
            lines.push(entry);
        }
    }
    std::fs::write(file, format!("{}\n", lines.join("\n")))?;
    Ok(())
}
