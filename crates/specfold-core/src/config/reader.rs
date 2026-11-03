//! Finds and loads spec.toml, and answers questions about the loaded configuration.

use super::defaults::{self, CONFIG_FILE};
use super::paths;
use super::types::RawConfig;
use crate::model::registry::Registry;
use crate::workspace::vcs;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Config {
    /// Repository root: the directory holding spec.toml.
    pub root: PathBuf,
    pub raw: RawConfig,
    pub registry: Registry,
    /// True when no spec.toml was found and defaults are in use.
    pub implicit: bool,
}

impl Config {
    /// Finds spec.toml in `start` or its ancestors. Without one, the nearest git root (or `start`)
    /// is used with defaults, so the tool works in any directory.
    pub fn discover(start: &Path) -> Result<Self> {
        let start = start.canonicalize().with_context(|| format!("cannot open {}", start.display()))?;
        let mut dir = Some(start.as_path());
        while let Some(d) = dir {
            let candidate = d.join(CONFIG_FILE);
            if candidate.is_file() {
                return Self::load(&candidate);
            }
            dir = d.parent();
        }
        let root = vcs::git_root(&start).unwrap_or(start);
        // F1: a public repository whose documents live in a mapped private directory.
        if let Some(private) = super::private_root::mapped(&root) {
            let toml = private.join(CONFIG_FILE);
            if toml.is_file() {
                return Self::load(&toml);
            }
            bail!(
                "{} is mapped to {} in {}, which has no {CONFIG_FILE}; next: spec -C {} init --write",
                root.display(),
                private.display(),
                super::private_root::file().unwrap_or_default().display(),
                private.display()
            );
        }
        Ok(Self::from_raw(root, RawConfig::default(), true))
    }

    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
        let raw: RawConfig = match toml::from_str(&text) {
            Ok(r) => r,
            Err(e) => {
                let (_, changes) = super::upgrade::plan(&text, super::upgrade::RENAMED);
                let hint =
                    if changes.is_empty() { String::new() } else { "; renamed keys: spec init --upgrade".into() };
                return Err(anyhow::Error::new(e).context(format!("invalid {}{hint}", path.display())));
            }
        };
        super::validate::loaded(path, &text, &raw)?;
        let root = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        Ok(Self::from_raw(root, raw, false))
    }

    pub fn from_raw(root: PathBuf, mut raw: RawConfig, implicit: bool) -> Self {
        if raw.spec.submodule_remotes {
            for (name, path) in submodule_specs(&root) {
                raw.remotes.entry(name).or_insert(crate::config::types::RemoteRoot { path, paths: vec![], auto: true });
            }
        }
        // C1: a nested project is a remote whether or not submodule_remotes is set.
        for (name, path) in crate::workspace::nested::remotes(&root, &raw.owners) {
            raw.remotes.entry(name).or_insert(crate::config::types::RemoteRoot { path, paths: vec![], auto: true });
        }
        for (name, dir) in super::private_root::prefixes() {
            let path = dir.to_string_lossy().into_owned();
            raw.remotes.entry(name).or_insert(crate::config::types::RemoteRoot { path, paths: vec![], auto: false });
        }
        let registry = Registry::new(&raw.types);
        Config { root, raw, registry, implicit }
    }

    pub fn spec_dir(&self) -> &str {
        &self.raw.spec.dir
    }

    pub fn manifests(&self) -> Vec<String> {
        match &self.raw.owners.manifests {
            Some(m) => m.clone(),
            None => defaults::MANIFESTS.iter().map(|s| s.to_string()).collect(),
        }
    }

    pub fn root_owner(&self) -> &str {
        &self.raw.owners.root
    }

    pub fn local_dir(&self) -> &str {
        &self.raw.local.path
    }

    pub fn expand_path(&self, p: &str) -> PathBuf {
        paths::expand(&self.root, p)
    }
}

/// Submodules listed in `.gitmodules` that carry their own spec.toml, as (remote name, path).
fn submodule_specs(root: &Path) -> Vec<(String, String)> {
    let Ok(text) = std::fs::read_to_string(root.join(".gitmodules")) else { return vec![] };
    let mut out = vec![];
    for line in text.lines() {
        let Some((k, v)) = line.split_once('=') else { continue };
        if k.trim() != "path" {
            continue;
        }
        let path = v.trim().trim_matches('"').trim_end_matches('/').to_string();
        let name = path.rsplit('/').next().unwrap_or(&path).to_string();
        if !name.is_empty() && root.join(&path).join("spec.toml").is_file() {
            out.push((name, path));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_config() {
        let raw: RawConfig = toml::from_str(
            r#"
[spec]
schema = 1
[owners]
root = "ws"
retired = ["services/old"]
[[owners.kinds]]
pattern = "services/*"
id = "{1}"
[public]
paths = ["libs/open"]
[public.companion]
"libs/open" = "services/open"
[external.notes]
path = "~/notes"
[files.store]
kind = "dir"
path = "/tmp/store"
[types.adr]
statuses = ["draft", "active"]
[types.memo]
key = "slug"
[[guards]]
name = "public"
paths = ["libs/open/**"]
deny_terms = ["internal-name"]
"#,
        )
        .unwrap();
        let cfg = Config::from_raw(PathBuf::from("/r"), raw, false);
        assert_eq!(cfg.root_owner(), "ws");
        assert!(cfg.registry.get("memo").is_some());
        assert_eq!(cfg.registry.get("adr").unwrap().statuses.len(), 2);
        assert_eq!(cfg.raw.guards[0].range, "staged");
    }

    #[test]
    fn unknown_keys_are_refused() {
        assert!(toml::from_str::<RawConfig>("[spec]\nschema = 1\ntypo = 2\n").is_err());
    }
}
