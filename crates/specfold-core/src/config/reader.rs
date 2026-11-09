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
    /// F1: the repository the configuration was found from: the spec home, or under roots.toml
    /// the public repository mapped to it. Code paths typed there are named from it.
    pub repo_root: PathBuf,
    /// The spec home: the directory holding spec.toml, its owners and documents. Every
    /// repository-relative path in the configuration and the corpus is relative to it.
    pub spec_home: PathBuf,
    pub raw: RawConfig,
    pub registry: Registry,
    /// True when no spec.toml was found and defaults are in use.
    pub implicit: bool,
    /// When set, walks from the spec home read only these files, plus the local directory and
    /// anything outside the repository (`spec check` sets it for a full run).
    pub tracked: Option<std::sync::Arc<vcs::TrackedSet>>,
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
                let repo_root = root.canonicalize().unwrap_or(root);
                return Self::load(&toml).map(|c| Config { repo_root, ..c });
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
            for (name, path) in crate::workspace::projects::submodule_remotes(&root) {
                raw.remotes.entry(name).or_insert(crate::config::types::RemoteRoot { path, paths: vec![], auto: true });
            }
        }
        // C1: a nested project is a remote whether or not submodule_remotes is set.
        for (name, path) in crate::workspace::projects::nested_remotes(&root, &raw.owners) {
            raw.remotes.entry(name).or_insert(crate::config::types::RemoteRoot { path, paths: vec![], auto: true });
        }
        for (name, dir) in super::private_root::prefixes() {
            let path = dir.to_string_lossy().into_owned();
            raw.remotes.entry(name).or_insert(crate::config::types::RemoteRoot { path, paths: vec![], auto: false });
        }
        let registry = Registry::new(&raw.types);
        Config { repo_root: root.clone(), spec_home: root, raw, registry, implicit, tracked: None }
    }

    /// The same configuration with walks kept to the files git tracks; unchanged outside git.
    pub fn tracked_only(mut self) -> Self {
        self.tracked = vcs::TrackedSet::load(&self.spec_home).map(std::sync::Arc::new);
        self
    }

    /// Whether a walk from the spec home reads the repository path `rel` (a file, or a folder
    /// when `dir`): everything without `tracked`, else tracked files and the local directory.
    pub fn reads(&self, rel: &str, dir: bool) -> bool {
        let Some(t) = &self.tracked else { return true };
        let local = self.local_dir();
        if rel.is_empty() || crate::paths::is_under(rel, local) {
            return true;
        }
        if dir { t.has_dir(rel) || t.has_file(rel) } else { t.has_file(rel) }
    }

    pub fn spec_dir(&self) -> &str {
        &self.raw.spec.dir
    }

    /// `(directory, type)` for each type kept in a directory of its own (`[types.<name>] dir`),
    /// the directory relative to an owner's root.
    pub fn type_dirs(&self) -> Vec<(String, String)> {
        let own =
            |t: &crate::model::TypeDef| Some((super::type_dirs::normalize(t.dir.as_deref()?).ok()?, t.name.clone()));
        self.registry.types.iter().filter_map(own).collect()
    }

    /// The regex that finds plain-text paths to documents: into the spec directory, or to a
    /// document file in a type's own directory.
    pub fn path_cite_re(&self) -> regex::Regex {
        crate::model::markdown::body::path_cite_re_with(self.spec_dir(), &self.type_dirs())
    }

    /// What a scan's path citations depend on, so a cache of scans is dropped when it changes.
    pub fn cite_key(&self) -> String {
        let mut key = self.spec_dir().to_string();
        for (dir, ty) in self.type_dirs() {
            key.push_str(&format!("\n{dir}={ty}"));
        }
        key
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
        paths::expand(&self.spec_home, p)
    }
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
