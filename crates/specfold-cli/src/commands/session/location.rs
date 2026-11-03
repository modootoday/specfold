//! Where a command runs: its directory, -C as typed, the repository it is in and the spec root,
//! read once per command; every path a command is given is resolved here.

use crate::cli::args::global::Global;
use anyhow::{Result, bail};
use specfold_core::Config;
use specfold_core::config::clone_path::{self, ClonePath};
use specfold_core::paths::{real_with_missing_tail, rel_of};
use std::path::{Path, PathBuf};

/// The directories one command works from.
#[derive(Debug, Clone, Default)]
pub struct Location {
    /// The directory the command runs in: -C made absolute, else the process directory.
    pub cwd: PathBuf,
    /// -C as typed, so commands to paste name it the same way.
    pub dir_as_typed: Option<PathBuf>,
    /// The repository the command runs in: the spec root, or under roots.toml the public
    /// repository mapped to it.
    pub repo_root: Option<PathBuf>,
    /// The spec root: the directory holding spec.toml.
    pub spec_home: Option<PathBuf>,
    /// The configuration found from `cwd`, so the command does not look for it again.
    config: Option<Config>,
}

impl Location {
    /// The directory of a command started with `g`; `dir_as_typed` is -C before it was resolved.
    pub fn at(g: &Global, dir_as_typed: Option<PathBuf>) -> Self {
        match &g.dir {
            Some(d) => Location { cwd: d.clone(), dir_as_typed, ..Location::default() },
            None => Location { dir_as_typed, ..Location::process() },
        }
    }

    /// The process directory, with no -C and no configuration.
    pub fn process() -> Self {
        Location { cwd: std::env::current_dir().unwrap_or_default(), ..Location::default() }
    }

    /// The same, with the configuration found from `cwd` and the roots it names.
    pub fn discovered(self) -> Self {
        let start = self.cwd.clone();
        self.with_config_from(&start)
    }

    /// `spec show --in-repo <dir>`: the configuration of another repository, read from here.
    pub fn in_repo(&self, dir: &Path) -> Self {
        let loc = Location { cwd: self.cwd.clone(), dir_as_typed: Some(dir.to_path_buf()), ..Location::default() };
        loc.with_config_from(dir)
    }

    /// A read-only remote repository opened from this command: its root is both roots.
    pub fn for_remote(&self, root: &Path) -> Self {
        Location {
            cwd: self.cwd.clone(),
            dir_as_typed: self.dir_as_typed.clone(),
            repo_root: Some(root.to_path_buf()),
            spec_home: Some(root.to_path_buf()),
            config: None,
        }
    }

    fn with_config_from(self, start: &Path) -> Self {
        match Config::discover(start) {
            Ok(cfg) => {
                let mut loc = self.rooted(&cfg);
                loc.config = Some(cfg);
                loc
            }
            Err(_) => self,
        }
    }

    /// The location with the roots `cfg` names: its spec root, and the repository `cwd` is in.
    pub fn rooted(mut self, cfg: &Config) -> Self {
        if self.spec_home.as_ref() != Some(&cfg.root) {
            self.repo_root = Some(self.clone_root(cfg).unwrap_or_else(|| cfg.root.clone()));
            self.spec_home = Some(cfg.root.clone());
        }
        self
    }

    /// E1: the public repository holding `cwd` that roots.toml maps to the spec root.
    fn clone_root(&self, cfg: &Config) -> Option<PathBuf> {
        let cwd = self.cwd.canonicalize().ok()?;
        let home = cfg.root.canonicalize().ok()?;
        if cwd.starts_with(&home) {
            return None;
        }
        clone_path::clone_of(cfg, &cwd)
    }

    /// The configuration found for this command, when one was.
    pub fn config(&self) -> Option<&Config> {
        self.config.as_ref()
    }

    /// The location without its configuration, for a process that must read spec.toml afresh.
    pub fn without_config(&self) -> Self {
        Location { config: None, ..self.clone() }
    }

    /// The directory the command runs in, or why the process directory cannot be read.
    pub fn start(&self) -> std::io::Result<PathBuf> {
        if self.cwd.as_os_str().is_empty() { std::env::current_dir() } else { Ok(self.cwd.clone()) }
    }

    /// C161: a relative path argument is read from -C/--dir, as if started there; absolute ones as is.
    pub fn arg_path(&self, p: &Path) -> PathBuf {
        match &self.dir_as_typed {
            Some(_) if p.is_relative() && p.as_os_str() != "-" => self.cwd.join(p),
            _ => p.to_path_buf(),
        }
    }

    /// An absolute path for a file that may not exist yet: the nearest existing ancestor is resolved.
    pub fn absolute(&self, p: &Path) -> PathBuf {
        let p = self.cwd.join(p);
        real_with_missing_tail(&p).unwrap_or(p)
    }

    /// A path given on the command line, made relative to the spec root.
    pub fn repo_path(&self, p: &str) -> String {
        let abs = self.cwd.join(p);
        // C258: a path that does not exist yet resolves from its nearest existing ancestor, so a new
        // file named from a subfolder lands in that subfolder.
        let home = self.spec_home.as_deref().unwrap_or(Path::new(""));
        match real_with_missing_tail(&abs).and_then(|a| rel_of(home, &a)) {
            Some(r) => r,
            None => p.trim_start_matches("./").trim_end_matches('/').to_string(),
        }
    }

    /// The directory the command runs in, relative to the spec root, or through its public path
    /// inside a mapped public repository (E1); `None` at the root or outside it.
    pub fn here(&self, cfg: &Config) -> Option<String> {
        let cwd = self.cwd.canonicalize().ok()?;
        let root = cfg.root.canonicalize().ok()?;
        let rel = match rel_of(&root, &cwd) {
            Some(r) => r,
            None => match self.locate(cfg, &cwd) {
                ClonePath::Mapped(rel) => rel,
                _ => return None,
            },
        };
        (!rel.is_empty()).then_some(rel)
    }

    /// `abs` named the way the spec root names it, through the public repository it lies in.
    fn locate(&self, cfg: &Config, abs: &Path) -> ClonePath {
        match self.repo_root.as_deref().filter(|r| Some(*r) != self.spec_home.as_deref()) {
            Some(clone) if abs.starts_with(clone) => clone_path::locate_in(cfg, clone, abs),
            _ => clone_path::locate(cfg, abs),
        }
    }

    /// E1: a code path. Typed inside a public repository that roots.toml maps to this spec root, it
    /// is read below the `[public]` path that repository is a copy of, not from the spec root.
    pub fn code_path(&self, cfg: &Config, p: &str) -> Result<String> {
        let abs = if Path::new(p).is_absolute() { PathBuf::from(p) } else { self.start()?.join(p) };
        let Some(abs) = real_with_missing_tail(&abs).filter(|a| !a.starts_with(&cfg.root)) else {
            return Ok(self.repo_path(p));
        };
        match self.locate(cfg, &abs) {
            ClonePath::Outside => Ok(self.repo_path(p)),
            ClonePath::Mapped(rel) => Ok(rel),
            ClonePath::Unmapped { rel, .. } if cfg.root.join(&rel).exists() => Ok(rel),
            ClonePath::Unmapped { clone, rel } => bail!(
                "'{p}' is {rel} in {}, which {} maps to {}, but no [public] path there is named for that repository and {rel} does not exist under the spec root; add public = \"<its [public] path>\" to that [[root]] entry, or give the path as the spec root names it; next: spec explain config",
                clone.display(),
                specfold_core::config::private_root::file().unwrap_or_default().display(),
                cfg.root.display()
            ),
        }
    }

    /// C130: a path a document can govern: inside the repository, below its root.
    pub fn governed_path(&self, cfg: &Config, p: &str) -> Result<String> {
        let rel = self.code_path(cfg, p)?;
        if p.trim().is_empty() || rel.is_empty() || rel.starts_with('/') || rel.split('/').any(|c| c == "..") {
            bail!("'{p}' is not a path inside the repository; give a file or folder below {}", cfg.root.display());
        }
        Ok(rel)
    }

    /// `..`, `../..` … from the command's directory up to the spec root, when it is below it.
    pub fn up_to_root(&self) -> Option<String> {
        let cwd = self.cwd.canonicalize().ok()?;
        let root = self.spec_home.as_ref()?.canonicalize().ok()?;
        let below = cwd.strip_prefix(&root).ok()?;
        let n = below.components().count();
        (n > 0).then(|| vec![".."; n].join("/"))
    }

    /// The spec root written the way -C was: `my repo/src` gives `my repo`, `.` when -C is the
    /// root, otherwise the absolute root.
    pub fn root_as_typed(&self) -> Option<PathBuf> {
        let dir = self.dir_as_typed.as_ref()?;
        let root = self.spec_home.as_ref()?;
        let below = self.cwd.strip_prefix(root).ok().map(|r| r.to_path_buf());
        Some(match below {
            Some(b) if b.as_os_str().is_empty() => dir.to_path_buf(),
            Some(b) => {
                let mut up = dir.to_path_buf();
                for _ in b.components() {
                    if !up.pop() {
                        return Some(root.to_path_buf());
                    }
                }
                if up.as_os_str().is_empty() { ".".into() } else { up }
            }
            None => root.to_path_buf(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(cwd: &Path, typed: Option<&str>, home: Option<&Path>) -> Location {
        Location {
            cwd: cwd.to_path_buf(),
            dir_as_typed: typed.map(PathBuf::from),
            repo_root: home.map(Path::to_path_buf),
            spec_home: home.map(Path::to_path_buf),
            config: None,
        }
    }

    #[test]
    fn root_as_typed_climbs_the_typed_dir_to_the_spec_root() {
        let root = Path::new("/r/my repo");
        let cases: &[(&str, &str, Option<&str>)] = &[
            ("/r/my repo", "my repo", Some("my repo")),
            ("/r/my repo/src", "my repo/src", Some("my repo")),
            ("/r/my repo/src", "src", Some(".")),
            ("/r/my repo/a/b", "b", Some("/r/my repo")),
            ("/elsewhere", "x", Some("/r/my repo")),
        ];
        for (cwd, typed, want) in cases {
            let got = at(Path::new(cwd), Some(typed), Some(root)).root_as_typed();
            assert_eq!(got.as_deref(), want.map(Path::new), "{cwd} typed {typed}");
        }
        assert_eq!(at(Path::new("/r/my repo"), None, Some(root)).root_as_typed(), None);
        assert_eq!(at(Path::new("/r/my repo"), Some("x"), None).root_as_typed(), None);
    }

    #[test]
    fn arg_path_reads_relative_paths_from_dir_only_when_given() {
        let cases: &[(Option<&str>, &str, &str)] = &[
            (Some("d"), "a.md", "/w/a.md"),
            (Some("d"), "-", "-"),
            (Some("d"), "/abs/a.md", "/abs/a.md"),
            (None, "a.md", "a.md"),
        ];
        for (typed, p, want) in cases {
            assert_eq!(at(Path::new("/w"), *typed, None).arg_path(Path::new(p)), PathBuf::from(want), "{p}");
        }
    }

    #[test]
    fn spec_root_paths_are_relative_to_the_spec_home() {
        let t = tempfile::tempdir().unwrap();
        let home = t.path().canonicalize().unwrap();
        std::fs::create_dir_all(home.join("a/b")).unwrap();
        let loc = at(&home.join("a/b"), None, Some(&home));
        assert_eq!(loc.up_to_root().as_deref(), Some("../.."));
        assert_eq!(loc.repo_path("new.md"), "a/b/new.md");
        assert_eq!(loc.repo_path("../x/"), "a/x");
        assert_eq!(loc.repo_path("./gone/../x/"), "gone/../x");
        assert_eq!(at(&home, None, Some(&home)).up_to_root(), None);
        assert_eq!(loc.absolute(Path::new("n/m.md")), home.join("a/b/n/m.md"));
    }
}
