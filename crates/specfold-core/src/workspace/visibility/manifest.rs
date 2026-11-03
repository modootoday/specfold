//! The visibility manifest file: which submodules a hosting service serves publicly, measured
//! against one version of `.gitmodules`. Read by V1, written by `spec sync visibility`.

use crate::authoring::files::hash;
use serde_json::{Map, Value, json};
use std::path::Path;

/// One submodule as the hosting service reported it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub url: String,
    /// As the service spells it, `PUBLIC`, `PRIVATE` or `INTERNAL`.
    pub visibility: String,
}

impl Entry {
    /// Whether anyone can read the submodule.
    pub fn is_public(&self) -> bool {
        self.visibility.eq_ignore_ascii_case("public")
    }
}

/// The parsed manifest; submodules keep the order of the file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Manifest {
    pub gitmodules_sha256: String,
    pub measured_at: String,
    pub submodules: Vec<(String, Entry)>,
}

/// The SHA-256 of `.gitmodules` the manifest is measured against; empty when there is none.
pub fn gitmodules_sha256(root: &Path) -> String {
    hash::file(&root.join(".gitmodules")).map(|(sha, _)| sha).unwrap_or_default()
}

/// The manifest at `rel`, or None when it is missing or not a manifest.
pub fn read(root: &Path, rel: &str) -> Option<Manifest> {
    parse(&std::fs::read_to_string(root.join(rel)).ok()?)
}

/// A manifest from its JSON text: `gitmodulesSha256` and `submodules` are required.
pub fn parse(text: &str) -> Option<Manifest> {
    let v: Value = serde_json::from_str(text).ok()?;
    let gitmodules_sha256 = v.get("gitmodulesSha256")?.as_str()?.to_string();
    let measured_at = v.get("measuredAt").and_then(Value::as_str).unwrap_or_default().to_string();
    let mut submodules = vec![];
    for (path, e) in v.get("submodules")?.as_object()? {
        let visibility = e.get("visibility")?.as_str()?.to_string();
        let url = e.get("url").and_then(Value::as_str).unwrap_or_default().to_string();
        submodules.push((path.clone(), Entry { url, visibility }));
    }
    Some(Manifest { gitmodules_sha256, measured_at, submodules })
}

/// The file text: two-space JSON with a final newline, keys in a fixed order.
pub fn render(m: &Manifest) -> String {
    let mut subs = Map::new();
    for (path, e) in &m.submodules {
        subs.insert(path.clone(), json!({"url": e.url, "visibility": e.visibility}));
    }
    let mut top = Map::new();
    top.insert("gitmodulesSha256".into(), Value::String(m.gitmodules_sha256.clone()));
    top.insert("measuredAt".into(), Value::String(m.measured_at.clone()));
    top.insert("submodules".into(), Value::Object(subs));
    let text = serde_json::to_string_pretty(&Value::Object(top)).unwrap_or_default();
    format!("{text}\n")
}
