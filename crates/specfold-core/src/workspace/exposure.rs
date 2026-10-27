//! Which paths of the repository get published, and why: configured public paths, public
//! submodules from an offline visibility manifest, and what `[public] detect` finds (a docs/
//! site, a static-site generator, an npm package or a crate that would ship a spec directory).

use crate::config::Config;
use crate::files::hash;
use crate::workspace::owners::matcher;
use ignore::WalkBuilder;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exposed {
    pub path: String,
    pub reason: String,
}

/// What a `[public] detect` entry looks for.
pub const DETECTORS: &[&str] = &["pages", "site-generators", "npm-files", "cargo-include"];

const SITE_FILES: &[&str] = &[
    "mkdocs.yml",
    "docusaurus.config.js",
    "docusaurus.config.ts",
    "_config.yml",
    "netlify.toml",
    "vercel.json",
    "astro.config.mjs",
    "hugo.toml",
    "book.toml",
];

#[derive(Deserialize)]
struct Manifest {
    #[serde(rename = "gitmodulesSha256")]
    gitmodules_sha256: String,
    submodules: BTreeMap<String, ManifestEntry>,
}

#[derive(Deserialize)]
struct ManifestEntry {
    visibility: String,
}

#[derive(Debug, Default)]
pub struct Report {
    pub exposed: Vec<Exposed>,
    /// Problems with the inputs themselves, as (path, message).
    pub problems: Vec<(String, String)>,
}

fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() { name.to_string() } else { format!("{dir}/{name}") }
}

fn manifest(cfg: &Config, rel: &str, out: &mut Report) {
    let read = std::fs::read_to_string(cfg.root.join(rel));
    let parsed = read.ok().and_then(|t| serde_json::from_str::<Manifest>(&t).ok());
    let Some(m) = parsed else {
        out.problems.push((rel.to_string(), format!("{rel} is missing or not a visibility manifest")));
        return;
    };
    let actual = hash::file(&cfg.root.join(".gitmodules")).map(|(sha, _)| sha).unwrap_or_default();
    if actual != m.gitmodules_sha256 {
        out.problems.push((rel.to_string(), format!("{rel} is older than .gitmodules; write it again")));
    }
    for (path, e) in m.submodules {
        if e.visibility.eq_ignore_ascii_case("public") {
            out.exposed.push(Exposed { path, reason: format!("a public submodule in {rel}") });
        }
    }
}

/// Whether an include-style list (npm `files`, Cargo `include`) takes in `spec_dir`.
fn list_takes(list: &[String], spec_dir: &str) -> bool {
    let probe = format!("{spec_dir}/x.md");
    let hit = |e: &str| {
        let e = e.trim_start_matches("./").trim_end_matches('/');
        e == "." || e == spec_dir || matcher(e).is_some_and(|m| m.is_match(&probe))
    };
    let taken = list.iter().filter(|e| !e.starts_with('!')).any(|e| hit(e));
    let dropped = list.iter().filter_map(|e| e.strip_prefix('!')).any(hit);
    taken && !dropped
}

fn npm(cfg: &Config, dir: &str, text: &str, out: &mut Report) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else { return };
    if v.get("private").and_then(|p| p.as_bool()) == Some(true) {
        return;
    }
    let spec = cfg.spec_dir();
    match v.get("files").and_then(|f| f.as_array()) {
        Some(files) => {
            let list: Vec<String> = files.iter().filter_map(|f| f.as_str().map(str::to_string)).collect();
            if list_takes(&list, spec) {
                out.exposed.push(Exposed {
                    path: join(dir, spec),
                    reason: format!("npm files in {}", join(dir, "package.json")),
                });
            }
        }
        None => {
            let ignore = std::fs::read_to_string(cfg.root.join(join(dir, ".npmignore"))).unwrap_or_default();
            if !ignore.lines().any(|l| l.trim().trim_matches('/') == spec) {
                out.exposed.push(Exposed {
                    path: dir.to_string(),
                    reason: format!("{} has no files list", join(dir, "package.json")),
                });
            }
        }
    }
}

fn cargo(cfg: &Config, dir: &str, text: &str, out: &mut Report) {
    let Ok(v) = text.parse::<toml::Table>() else { return };
    let Some(pkg) = v.get("package").and_then(|p| p.as_table()) else { return };
    if pkg.get("publish").and_then(|p| p.as_bool()) == Some(false) {
        return;
    }
    let list = |k: &str| -> Option<Vec<String>> {
        pkg.get(k).and_then(|a| a.as_array()).map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_string)).collect())
    };
    let spec = cfg.spec_dir();
    let shipped = match (list("include"), list("exclude")) {
        (Some(inc), _) => list_takes(&inc, spec),
        (None, Some(exc)) => !list_takes(&exc, spec),
        (None, None) => true,
    };
    if shipped {
        out.exposed
            .push(Exposed { path: join(dir, spec), reason: format!("a published crate, {}", join(dir, "Cargo.toml")) });
    }
}

fn detect(cfg: &Config, out: &mut Report) {
    let on = |d: &str| cfg.raw.public.detect.iter().any(|x| x == d);
    if on("pages") && cfg.root.join("docs").is_dir() {
        out.exposed.push(Exposed { path: "docs".into(), reason: "docs/ is what GitHub Pages publishes".into() });
    }
    if !(on("site-generators") || on("npm-files") || on("cargo-include")) {
        return;
    }
    for e in WalkBuilder::new(&cfg.root).hidden(false).build().flatten() {
        if !e.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let name = e.file_name().to_string_lossy().to_string();
        let Some(dir) = e.path().parent().and_then(|p| p.strip_prefix(&cfg.root).ok()) else { continue };
        let dir = dir.to_string_lossy().replace('\\', "/");
        if on("site-generators") && SITE_FILES.contains(&name.as_str()) {
            let path = if dir.is_empty() { ".".to_string() } else { dir.clone() };
            out.exposed.push(Exposed { path, reason: format!("a site generator ({name})") });
        }
        let read = || std::fs::read_to_string(e.path()).unwrap_or_default();
        if on("npm-files") && name == "package.json" {
            npm(cfg, &dir, &read(), out);
        }
        if on("cargo-include") && name == "Cargo.toml" {
            cargo(cfg, &dir, &read(), out);
        }
    }
}

pub fn scan(cfg: &Config) -> Report {
    let mut out = Report::default();
    for p in &cfg.raw.public.paths {
        out.exposed.push(Exposed { path: p.clone(), reason: "listed in [public] paths".into() });
    }
    if let Some(rel) = &cfg.raw.public.submodules_manifest {
        manifest(cfg, rel, &mut out);
    }
    for d in cfg.raw.public.detect.iter().filter(|d| !DETECTORS.contains(&d.as_str())) {
        out.problems
            .push(("spec.toml".into(), format!("[public] detect: no detector '{d}' ({})", DETECTORS.join(", "))));
    }
    detect(cfg, &mut out);
    out.exposed.sort_by(|a, b| a.path.cmp(&b.path));
    out.exposed.dedup_by(|a, b| a.path == b.path);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn include_lists() {
        let l = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(list_takes(&l(&["."]), ".spec"));
        assert!(list_takes(&l(&[".spec"]), ".spec"));
        assert!(list_takes(&l(&["**/*.md"]), ".spec"));
        assert!(!list_takes(&l(&["dist"]), ".spec"));
        assert!(!list_takes(&l(&[".", "!.spec"]), ".spec"));
    }
}
