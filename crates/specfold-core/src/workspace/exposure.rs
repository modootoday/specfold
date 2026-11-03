//! Which paths of the repository get published, and why: configured public paths, public
//! submodules from an offline visibility manifest, and what `[public] detect` finds (a docs/
//! site, a static-site generator, an npm package or a crate that would ship a spec directory or
//! a type's own directory).

use crate::config::Config;
use crate::workspace::owners::matcher;
use crate::workspace::visibility::{self, WRITE_COMMAND};

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
    let Some(m) = visibility::manifest::read(&cfg.spec_home, rel) else {
        let msg = format!("{rel} is missing or not a visibility manifest; next: {WRITE_COMMAND}");
        out.problems.push((rel.to_string(), msg));
        return;
    };
    if visibility::manifest::gitmodules_sha256(&cfg.spec_home) != m.gitmodules_sha256 {
        out.problems.push((rel.to_string(), format!("{rel} is older than .gitmodules; next: {WRITE_COMMAND}")));
    }
    for (path, e) in m.submodules {
        if e.is_public() {
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

/// The directories of a package at `dir` that hold documents: its spec directory, and each type
/// directory (`[types.<name>] dir`) that exists there. The spec directory comes first.
fn doc_dirs(cfg: &Config, dir: &str) -> Vec<String> {
    let own = cfg.type_dirs().into_iter().map(|(d, _)| d);
    let present = own.filter(|d| cfg.spec_home.join(join(dir, d)).is_dir());
    std::iter::once(cfg.spec_dir().to_string()).chain(present).collect()
}

fn npm(cfg: &Config, dir: &str, text: &str, out: &mut Report) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else { return };
    if v.get("private").and_then(|p| p.as_bool()) == Some(true) {
        return;
    }
    let pkg = join(dir, "package.json");
    match v.get("files").and_then(|f| f.as_array()) {
        Some(files) => {
            let list: Vec<String> = files.iter().filter_map(|f| f.as_str().map(str::to_string)).collect();
            for d in doc_dirs(cfg, dir).iter().filter(|d| list_takes(&list, d)) {
                out.exposed.push(Exposed { path: join(dir, d), reason: format!("npm files in {pkg}") });
            }
        }
        None => {
            let ignore = std::fs::read_to_string(cfg.spec_home.join(join(dir, ".npmignore"))).unwrap_or_default();
            let ignored = |d: &str| ignore.lines().any(|l| l.trim().trim_matches('/') == d);
            let shipped: Vec<String> = doc_dirs(cfg, dir).into_iter().filter(|d| !ignored(d)).collect();
            // A shipped spec directory exposes the whole package, as before type directories.
            let paths = match shipped.first() {
                Some(d) if d == cfg.spec_dir() => vec![dir.to_string()],
                _ => shipped.iter().map(|d| join(dir, d)).collect(),
            };
            for path in paths {
                out.exposed.push(Exposed { path, reason: format!("{pkg} has no files list") });
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
    let (include, exclude) = (list("include"), list("exclude"));
    let shipped = |d: &str| match (&include, &exclude) {
        (Some(inc), _) => list_takes(inc, d),
        (None, Some(exc)) => !list_takes(exc, d),
        (None, None) => true,
    };
    for d in doc_dirs(cfg, dir).iter().filter(|d| shipped(d)) {
        out.exposed
            .push(Exposed { path: join(dir, d), reason: format!("a published crate, {}", join(dir, "Cargo.toml")) });
    }
}

fn detect(cfg: &Config, out: &mut Report) {
    let on = |d: &str| cfg.raw.public.detect.iter().any(|x| x == d);
    if on("pages") && cfg.spec_home.join("docs").is_dir() {
        out.exposed.push(Exposed { path: "docs".into(), reason: "docs/ is what GitHub Pages publishes".into() });
    }
    if !(on("site-generators") || on("npm-files") || on("cargo-include")) {
        return;
    }
    let walk = super::walk(cfg, &cfg.spec_home).build();
    for e in walk.flatten() {
        if !e.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let name = e.file_name().to_string_lossy().to_string();
        let Some(dir) = e.path().parent().and_then(|p| p.strip_prefix(&cfg.spec_home).ok()) else { continue };
        let dir = crate::paths::slash(dir);
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
    // C1: a nested project publishes itself under its own rules and checks.
    out.exposed.retain(|e| !cfg.raw.owners.in_nested(&crate::workspace::placement::literal_base(&e.path)));
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
