//! What a run reads from the repository: tracked files in a full run (S9), never `.git`, and
//! moved gitlinks read as one split or one pin bump (D1).

#[path = "../common/mod.rs"]
mod common;

use serde_json::Value;

/// `spec check --json` with the tracked-only default the fixtures turn off.
fn check_tracked(r: &common::Repo, args: &[&str]) -> (i32, Value) {
    let out = r.spec().env_remove("SPECFOLD_INCLUDE_UNTRACKED").arg("check").args(args).arg("--json").output().unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    (out.status.code().unwrap_or(-1), v)
}

fn rules(v: &Value) -> Vec<String> {
    v["items"].as_array().unwrap().iter().map(|f| f["rule"].as_str().unwrap().to_string()).collect()
}

const BROKEN: &str = "---\nid: adr:ws/wrong\nstatus: draft\ncreated: 2026-01-01\nupdated: 2026-01-01\n---\n# Draft\n";

#[test]
fn s9_full_check_leaves_untracked_documents_out_with_one_warning() {
    let r = common::repo();
    r.write(".spec/adr/20260101000000-one.adr.md", BROKEN);
    r.write(".spec/adr/20260101000001-two.adr.md", BROKEN);
    let (code, v) = check_tracked(&r, &[]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(rules(&v), ["S9"], "{v}");
    let f = &v["items"][0];
    assert!(f["message"].as_str().unwrap().starts_with("2 untracked document(s)"), "{f}");
    assert_eq!(f["candidates"].as_array().unwrap().len(), 2, "{f}");
    let (code, v) = check_tracked(&r, &["--include-untracked"]);
    assert_eq!(code, 1, "{v}");
    assert!(rules(&v).contains(&"S2".to_string()) && !rules(&v).contains(&"S9".to_string()), "{v}");
}

#[test]
fn s9_staged_and_named_runs_still_read_untracked_documents() {
    let r = common::repo();
    r.write(".spec/adr/20260101000000-one.adr.md", BROKEN);
    let (code, v) = check_tracked(&r, &[".spec/adr/20260101000000-one.adr.md"]);
    assert_eq!(code, 1, "a named file is checked as it is: {v}");
    r.git(&["add", ".spec/adr/20260101000000-one.adr.md"]);
    let (code, v) = check_tracked(&r, &[]);
    assert_eq!(code, 1, "an added document is tracked: {v}");
    assert!(!rules(&v).contains(&"S9".to_string()), "{v}");
}

#[test]
fn s9_untracked_forbidden_paths_and_git_internals_are_not_read() {
    let r = common::repo();
    r.write("spec.toml", &(r.read("spec.toml") + "[forbid]\npaths = [\"old-docs\", \"**/old-docs\"]\n"));
    r.commit("forbid");
    r.write(".git/old-docs/a.md", "x");
    r.write("old-docs/b.md", "x");
    let (code, v) = check_tracked(&r, &["--rule", "Z1"]);
    assert_eq!(code, 0, "{v}");
    let (code, v) = check_tracked(&r, &["--rule", "Z1", "--include-untracked"]);
    assert_eq!(code, 1, "{v}");
    let paths: Vec<&str> = v["items"].as_array().unwrap().iter().map(|f| f["path"].as_str().unwrap()).collect();
    assert!(!paths.is_empty() && paths.iter().all(|p| p.starts_with("old-docs")), ".git is never read: {v}");
}

#[test]
fn s9_the_local_directory_is_still_read_in_a_full_run() {
    let r = common::repo();
    r.write(".spec.local/ws/research/20260101000000-note.research.md", "---\nid: research:ws/wrong\n---\n# Note\n");
    let (_, v) = check_tracked(&r, &[]);
    assert!(rules(&v).contains(&"S2".to_string()), "{v}");
    assert!(!rules(&v).contains(&"S9".to_string()), "{v}");
}

fn git_in(dir: &std::path::Path, args: &[&str]) {
    let ok = std::process::Command::new("git")
        .args(["-c", "user.email=t@t", "-c", "user.name=t", "-c", "protocol.file.allow=always"])
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap()
        .success();
    assert!(ok, "git {args:?}");
}

/// vendor/lib/src with a.rs, b.rs and d.rs under an active document, committed; then the folder
/// replaced, staged, by a submodule holding a.rs unchanged, b.rs changed and c.rs new.
fn split_repo() -> (common::Repo, common::Repo, String) {
    let lib = common::Repo::new(tempfile::tempdir().unwrap());
    git_in(lib.path(), &["init", "-q", "-b", "main"]);
    lib.write("src/a.rs", "fn a() {}\n");
    lib.write("src/b.rs", "fn b() { 2 }\n");
    lib.write("src/c.rs", "fn c() {}\n");
    git_in(lib.path(), &["add", "-A"]);
    git_in(lib.path(), &["commit", "-qm", "lib"]);
    let r = common::repo();
    for f in ["a", "b", "d"] {
        r.write(&format!("vendor/lib/src/{f}.rs"), &format!("fn {f}() {{}}\n"));
    }
    let id = r.new_doc("sot", "vendor/lib/src", "Vendored lib");
    assert_eq!(r.json(&["set", &id, "status=active", "--write"]).0, 0);
    r.commit("vendored");
    r.git(&["rm", "-r", "-q", "vendor/lib"]);
    git_in(r.path(), &["submodule", "add", "-q", lib.path().to_str().unwrap(), "vendor/lib"]);
    (r, lib, id)
}

#[test]
fn d1_a_folder_split_into_a_submodule_is_one_change_not_removals() {
    let (r, _lib, _) = split_repo();
    let (_, v) = r.json(&["check", "--staged", "--rule", "D1"]);
    let split = &v["gitlinks"][0];
    assert_eq!(split["kind"], "split", "{v}");
    assert_eq!((split["moved"].as_u64(), split["changed"].as_u64()), (Some(1), Some(2)), "{v}");
    let items = v["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{v}");
    let msg = items[0]["message"].as_str().unwrap();
    assert!(msg.starts_with("vendor/lib (new submodule at "), "{msg}");
    assert!(msg.contains("1 file(s) moved in unchanged, 2 new or changed"), "{msg}");
    assert!(msg.contains("vendor/lib/src/d.rs (removed)"), "a file the submodule lacks is still removed: {msg}");
    assert!(!msg.contains("a.rs") && !msg.contains("b.rs"), "{msg}");
}

#[test]
fn d1_a_pin_bump_is_named_once_for_every_file_inside() {
    let (r, lib, _) = split_repo();
    r.commit("split");
    let sub = r.path().join("vendor/lib");
    std::fs::write(sub.join("src/a.rs"), "fn a() { 1 }\n").unwrap();
    std::fs::write(sub.join("src/c.rs"), "fn c() { 1 }\n").unwrap();
    git_in(&sub, &["commit", "-qam", "bump"]);
    r.git(&["add", "vendor/lib"]);
    let (_, v) = r.json(&["check", "--staged", "--rule", "D1"]);
    assert_eq!(v["gitlinks"][0]["kind"], "bump", "{v}");
    assert_eq!(v["gitlinks"][0]["changed"], 2, "{v}");
    let items = v["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{v}");
    let msg = items[0]["message"].as_str().unwrap();
    assert!(msg.starts_with("vendor/lib (pin ") && msg.contains("2 file(s) changed inside"), "{msg}");
    assert!(!msg.contains("src/a.rs") && !msg.contains("src/c.rs"), "{msg}");
    drop(lib);
}
