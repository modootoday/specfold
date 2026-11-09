//! What a run reads from the repository: tracked files in a full run (S9), and never `.git`.

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
