//! A throwaway repository for CLI tests: two apps, one library, a git history.

#![allow(dead_code)]

use assert_cmd::Command;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub struct Repo {
    pub dir: tempfile::TempDir,
}

impl Repo {
    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    pub fn write(&self, rel: &str, text: &str) {
        let p = self.path().join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    pub fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.path().join(rel)).unwrap()
    }

    pub fn git(&self, args: &[&str]) {
        let ok =
            std::process::Command::new("git").args(args).current_dir(self.path()).output().unwrap().status.success();
        assert!(ok, "git {args:?} failed");
    }

    pub fn commit(&self, msg: &str) {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-qm", msg]);
    }

    pub fn spec(&self) -> Command {
        let mut c = Command::cargo_bin("spec").unwrap();
        c.current_dir(self.path())
            .env("SPECFOLD_CACHE", self.path().join(".cache"))
            .timeout(std::time::Duration::from_secs(30));
        c
    }

    /// Runs with --json and returns (exit code, parsed output).
    pub fn json(&self, args: &[&str]) -> (i32, Value) {
        let out = self.spec().args(args).arg("--json").output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        let v: Value =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("spec {args:?} printed non-JSON ({e}): {text}"));
        (out.status.code().unwrap_or(-1), v)
    }

    pub fn text(&self, args: &[&str]) -> (i32, String) {
        let out = self.spec().args(args).output().unwrap();
        (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// Creates a document and returns its id.
    pub fn new_doc(&self, ty: &str, governs: &str, title: &str) -> String {
        let (code, v) = self.json(&["new", ty, "--governs", governs, "--title", title]);
        assert_eq!(code, 0, "{v}");
        v["id"].as_str().unwrap().to_string()
    }
}

pub fn repo() -> Repo {
    let dir = tempfile::tempdir().unwrap();
    let r = Repo { dir };
    r.git(&["init", "-q", "-b", "main"]);
    r.git(&["config", "user.email", "test@example.com"]);
    r.git(&["config", "user.name", "tester"]);
    r.write("services/shop/src/cart/total.rs", "fn total() {}\n");
    r.write("services/blog/src/main.rs", "fn main() {}\n");
    r.write("libs/auth/Cargo.toml", "[package]\nname = \"auth\"\n");
    r.write("libs/open/go.mod", "module open\n");
    r.write(
        "spec.toml",
        "[spec]\nschema = 1\n[owners]\nroot = \"ws\"\n[[owners.kinds]]\npattern = \"services/*\"\nid = \"{1}\"\n[[owners.kinds]]\npattern = \"libs/*\"\nid = \"lib.{1}\"\nmanifest = true\n[public]\npaths = [\"libs/open\"]\n[public.companion]\n\"libs/open\" = \"libs/auth\"\n[aliases]\nfile = \".spec/_aliases.tsv\"\n",
    );
    r.write(".gitignore", ".cache/\n.spec.local/\n");
    r.commit("init");
    r
}

pub fn bin_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_spec")).parent().unwrap().to_path_buf()
}
