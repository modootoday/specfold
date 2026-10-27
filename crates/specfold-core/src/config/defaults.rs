//! Default values for spec.toml, kept apart so the schema reads as data.

pub const CONFIG_FILE: &str = "spec.toml";

/// Manifests that make a directory a package owner in any common ecosystem.
pub const MANIFESTS: &[&str] = &[
    "package.json",
    "Cargo.toml",
    "go.mod",
    "pyproject.toml",
    "setup.py",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "composer.json",
    "Gemfile",
    "mix.exs",
    "Package.swift",
    "pubspec.yaml",
    "*.csproj",
    "*.fsproj",
    "deno.json",
];

pub fn schema() -> u32 {
    1
}

pub fn spec_dir() -> String {
    ".spec".into()
}

pub fn root_owner() -> String {
    "root".into()
}

pub fn local_dir() -> String {
    ".spec.local".into()
}

pub fn cite_check() -> String {
    "cite-check".into()
}

pub fn inline_max_bytes() -> u64 {
    262_144
}

pub fn guard_range() -> String {
    "staged".into()
}

pub fn guard_level() -> String {
    "error".into()
}

pub fn plugin_timeout_ms() -> u64 {
    30_000
}

pub fn verify_timeout_ms() -> u64 {
    60_000
}

pub fn stale_days() -> i64 {
    30
}

pub fn stalled_days() -> i64 {
    14
}

pub fn draft_days() -> i64 {
    30
}

pub fn open_item_patterns() -> Vec<String> {
    vec!["TODO".into(), "FIXME".into(), "TBD".into()]
}

pub fn history_patterns() -> Vec<String> {
    vec![r"(?i)\bmoved (to|from)\b".into(), r"(?i)\bformerly\b".into(), r"(?i)\brenamed (to|from)\b".into()]
}
