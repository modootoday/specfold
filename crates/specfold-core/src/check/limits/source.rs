//! `<file>#<key>`: a value inside another TOML or JSON file, so the file that owns a number or a
//! list is its only copy. A source that cannot be read or parsed is an error, never empty.

use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;
use std::path::Path;

/// The value `pointer` names under `root`: the file before `#`, then the dotted key after it,
/// the whole file without one. A `.json` file is JSON and a `.toml` file TOML; any other is read
/// as JSON when it starts with `{`, else as TOML (a TOML table header also starts with `[`).
pub fn read(root: &Path, pointer: &str) -> Result<Value> {
    let (file, key) = pointer.split_once('#').unwrap_or((pointer, ""));
    if file.is_empty() {
        bail!("'{pointer}' names no file; write <file>#<key>");
    }
    let text = std::fs::read_to_string(root.join(file)).with_context(|| format!("cannot read {file}"))?;
    let json = file.ends_with(".json") || (!file.ends_with(".toml") && text.trim_start().starts_with('{'));
    let doc: Value = if json {
        serde_json::from_str(&text).with_context(|| format!("{file} is not valid JSON"))?
    } else {
        let t: toml::Value = toml::from_str(&text).with_context(|| format!("{file} is not valid TOML"))?;
        serde_json::to_value(t).with_context(|| format!("{file} cannot be read as data"))?
    };
    let mut at = &doc;
    for part in key.split('.').filter(|p| !p.is_empty()) {
        at = at.get(part).ok_or_else(|| anyhow!("{file} has no key '{key}' (missing '{part}')"))?;
    }
    Ok(at.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_table_by_dotted_key_and_refuses_what_is_missing() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("tool.toml"), "[limits.files]\n\"a.md\" = { bytes = 10 }\n").unwrap();
        std::fs::write(tmp.path().join("lock"), "{\"outputs\": [{\"path\": \"x\"}]}").unwrap();
        let v = read(tmp.path(), "tool.toml#limits.files").unwrap();
        assert_eq!(v["a.md"]["bytes"], 10);
        assert_eq!(read(tmp.path(), "lock#outputs").unwrap()[0]["path"], "x");
        assert!(read(tmp.path(), "tool.toml#limits.dirs").unwrap_err().to_string().contains("no key"));
        assert!(read(tmp.path(), "nope.toml#x").unwrap_err().to_string().contains("cannot read"));
        std::fs::write(tmp.path().join("bad.toml"), "[x\n").unwrap();
        assert!(read(tmp.path(), "bad.toml").unwrap_err().to_string().contains("not valid TOML"));
    }
}
