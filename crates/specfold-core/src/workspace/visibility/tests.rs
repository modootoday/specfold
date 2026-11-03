//! The manifest writer against a stub forge: no network, no git.

use super::*;
use std::cell::RefCell;
use std::path::Path;

const REL: &str = "vis.json";
const NOW: &str = "2026-01-02T03:04:05Z";
const LATER: &str = "2026-02-03T04:05:06Z";

/// Answers from a fixed table; `offline` refuses every question; `asked` records each one.
struct Stub {
    table: Vec<(&'static str, &'static str)>,
    offline: bool,
    asked: RefCell<Vec<String>>,
}

impl Stub {
    fn new(table: &[(&'static str, &'static str)]) -> Stub {
        Stub { table: table.to_vec(), offline: false, asked: RefCell::new(vec![]) }
    }
}

impl Forge for Stub {
    fn name(&self) -> &str {
        "stub"
    }

    fn repository(&self, url: &str) -> Result<String, ForgeError> {
        repository_of(url).ok_or_else(|| ForgeError::Unsupported(format!("{url} is not a github.com repository URL")))
    }

    fn visibility(&self, repository: &str) -> Result<String, ForgeError> {
        self.asked.borrow_mut().push(repository.to_string());
        if self.offline {
            return Err(ForgeError::Unreachable("cannot reach the stub forge".into()));
        }
        let hit = self.table.iter().find(|(r, _)| *r == repository);
        hit.map(|(_, v)| v.to_string()).ok_or_else(|| ForgeError::Failed(format!("no repository {repository}")))
    }
}

fn gitmodules(root: &Path, subs: &[(&str, &str)]) {
    let text: String =
        subs.iter().map(|(path, url)| format!("[submodule \"{path}\"]\n\tpath = {path}\n\turl = {url}\n")).collect();
    std::fs::write(root.join(".gitmodules"), text).unwrap();
}

fn write(root: &Path, m: &Measured) {
    std::fs::write(root.join(&m.rel), &m.text).unwrap();
}

const SHOP: (&str, &str) = ("services/shop", "https://github.com/acme/shop.git");
const AUTH: (&str, &str) = ("libs/auth", "git@github.com:acme/auth.git");

#[test]
fn measure_without_a_manifest_writes_a_fresh_one() {
    let dir = tempfile::tempdir().unwrap();
    gitmodules(dir.path(), &[SHOP, AUTH]);
    let forge = Stub::new(&[("acme/shop", "PUBLIC"), ("acme/auth", "PRIVATE")]);
    let m = measure(dir.path(), REL, &forge, NOW).unwrap();
    assert!(m.changed && !m.exists);
    assert_eq!(m.added, vec!["services/shop", "libs/auth"]);
    let sha = manifest::gitmodules_sha256(dir.path());
    let expected = format!(
        "{{\n  \"gitmodulesSha256\": \"{sha}\",\n  \"measuredAt\": \"{NOW}\",\n  \"submodules\": {{\n    \"services/shop\": {{\n      \"url\": \"https://github.com/acme/shop.git\",\n      \"visibility\": \"PUBLIC\"\n    }},\n    \"libs/auth\": {{\n      \"url\": \"git@github.com:acme/auth.git\",\n      \"visibility\": \"PRIVATE\"\n    }}\n  }}\n}}\n"
    );
    assert_eq!(m.text, expected);
    assert_eq!(manifest::parse(&m.text), Some(m.after));
}

#[test]
fn measure_with_nothing_changed_keeps_the_file_bytes() {
    let dir = tempfile::tempdir().unwrap();
    gitmodules(dir.path(), &[SHOP, AUTH]);
    let forge = Stub::new(&[("acme/shop", "PUBLIC"), ("acme/auth", "PRIVATE")]);
    write(dir.path(), &measure(dir.path(), REL, &forge, NOW).unwrap());
    let before = std::fs::read(dir.path().join(REL)).unwrap();
    let again = measure(dir.path(), REL, &forge, LATER).unwrap();
    assert!(!again.changed);
    assert_eq!(again.text.as_bytes(), &before[..]);
    assert_eq!(again.after.measured_at, NOW);
}

#[test]
fn measure_adds_a_new_submodule_after_the_existing_order() {
    let dir = tempfile::tempdir().unwrap();
    gitmodules(dir.path(), &[AUTH]);
    let forge = Stub::new(&[("acme/shop", "PUBLIC"), ("acme/auth", "PRIVATE")]);
    write(dir.path(), &measure(dir.path(), REL, &forge, NOW).unwrap());
    gitmodules(dir.path(), &[SHOP, AUTH]);
    let m = measure(dir.path(), REL, &forge, LATER).unwrap();
    assert!(m.changed);
    assert_eq!(m.added, vec!["services/shop"]);
    let order: Vec<&str> = m.after.submodules.iter().map(|(p, _)| p.as_str()).collect();
    assert_eq!(order, vec!["libs/auth", "services/shop"]);
    assert_eq!(m.after.measured_at, LATER);
    assert_eq!(m.after.gitmodules_sha256, manifest::gitmodules_sha256(dir.path()));
}

#[test]
fn measure_drops_a_removed_submodule() {
    let dir = tempfile::tempdir().unwrap();
    gitmodules(dir.path(), &[SHOP, AUTH]);
    let forge = Stub::new(&[("acme/shop", "PUBLIC"), ("acme/auth", "PRIVATE")]);
    write(dir.path(), &measure(dir.path(), REL, &forge, NOW).unwrap());
    gitmodules(dir.path(), &[AUTH]);
    let m = measure(dir.path(), REL, &forge, LATER).unwrap();
    assert!(m.changed);
    assert_eq!(m.removed, vec!["services/shop"]);
    assert!(!m.text.contains("services/shop"), "{}", m.text);
}

#[test]
fn measure_reports_a_changed_visibility_as_updated() {
    let dir = tempfile::tempdir().unwrap();
    gitmodules(dir.path(), &[SHOP]);
    write(dir.path(), &measure(dir.path(), REL, &Stub::new(&[("acme/shop", "PRIVATE")]), NOW).unwrap());
    let m = measure(dir.path(), REL, &Stub::new(&[("acme/shop", "PUBLIC")]), LATER).unwrap();
    assert!(m.changed);
    assert_eq!(m.updated, vec!["services/shop"]);
}

#[test]
fn measure_refuses_a_url_off_the_forge_before_asking() {
    let dir = tempfile::tempdir().unwrap();
    gitmodules(dir.path(), &[SHOP, ("libs/pay", "https://git.example.com/acme/pay.git")]);
    let forge = Stub::new(&[("acme/shop", "PUBLIC")]);
    let err = measure(dir.path(), REL, &forge, NOW).unwrap_err();
    assert!(matches!(err, ForgeError::Unsupported(_)), "{err:?}");
    let msg = err.to_string();
    assert!(msg.contains("libs/pay") && msg.contains("git.example.com"), "{msg}");
    assert!(forge.asked.borrow().is_empty());
    assert!(!dir.path().join(REL).exists());
}

#[test]
fn measure_refuses_when_the_forge_is_unreachable() {
    let dir = tempfile::tempdir().unwrap();
    gitmodules(dir.path(), &[SHOP, AUTH]);
    let mut forge = Stub::new(&[("acme/shop", "PUBLIC"), ("acme/auth", "PRIVATE")]);
    forge.offline = true;
    let err = measure(dir.path(), REL, &forge, NOW).unwrap_err();
    assert!(matches!(err, ForgeError::Unreachable(_)), "{err:?}");
    assert_eq!(forge.asked.borrow().len(), 1, "the first refusal stops the run");
}

#[test]
fn manifest_parse_keeps_file_order_and_optional_fields() {
    let text = "{\"gitmodulesSha256\": \"0\", \"submodules\": {\"b\": {\"visibility\": \"PUBLIC\"}, \"a\": {\"visibility\": \"PRIVATE\"}}}";
    let m = manifest::parse(text).unwrap();
    let order: Vec<&str> = m.submodules.iter().map(|(p, _)| p.as_str()).collect();
    assert_eq!(order, vec!["b", "a"]);
    assert!(m.submodules[0].1.is_public() && m.measured_at.is_empty());
}
