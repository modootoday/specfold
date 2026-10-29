//! `spec check --format`: the documents CI systems read.

mod common;

use serde_json::Value;

fn warned() -> common::Repo {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Use files");
    let (code, _) = r.json(&["set", &id, "colour=blue"]);
    assert_eq!(code, 0);
    r
}

#[test]
fn github_format_prints_annotations_and_a_step_summary() {
    let r = warned();
    let summary = r.path().join("summary.md");
    let out = r.spec().args(["check", "--format", "github"]).env("GITHUB_STEP_SUMMARY", &summary).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(text.lines().any(|l| l.starts_with("::warning file=") && l.contains(",title=spec S1::")), "{text}");
    assert!(!text.contains("next:"), "machine formats carry no guidance lines: {text}");
    let s = std::fs::read_to_string(summary).unwrap();
    assert!(s.contains("| S1 | warning |"), "{s}");
    let path = text.split("file=").nth(1).unwrap().split(',').next().unwrap().to_string();
    let file = r.path().join(&path);
    let doc = std::fs::read_to_string(&file).unwrap();
    let doc: String = doc
        .lines()
        .map(|l| if l.starts_with("status:") { "status: bogus\n".to_string() } else { format!("{l}\n") })
        .collect();
    std::fs::write(&file, doc).unwrap();
    let out = r.spec().args(["check", "--format", "github"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(1), "errors still fail the run: {text}");
    assert!(text.contains("::error file=") && text.contains("title=spec S4::"), "{text}");
}

#[test]
fn sarif_gitlab_and_rdjson_are_whole_json_documents() {
    let r = warned();
    let (_, t) = r.text(&["check", "--format", "sarif"]);
    let v: Value = serde_json::from_str(&t).unwrap();
    assert_eq!(v["version"], "2.1.0");
    let res = &v["runs"][0]["results"][0];
    assert_eq!(res["ruleId"], "S1");
    assert_eq!(res["level"], "warning");
    assert!(res["partialFingerprints"]["specfold/v1"].is_string());
    let rules = v["runs"][0]["tool"]["driver"]["rules"].as_array().unwrap();
    assert_eq!(rules[res["ruleIndex"].as_u64().unwrap() as usize]["id"], "S1");

    let (_, t) = r.text(&["check", "--format", "gitlab"]);
    let v: Value = serde_json::from_str(&t).unwrap();
    assert_eq!(v[0]["check_name"], "S1");
    assert_eq!(v[0]["severity"], "minor");
    assert!(v[0]["location"]["lines"]["begin"].as_u64().unwrap() >= 1);

    let (_, t) = r.text(&["check", "--format", "rdjson"]);
    let v: Value = serde_json::from_str(&t).unwrap();
    assert_eq!(v["source"]["name"], "specfold");
    assert_eq!(v["diagnostics"][0]["code"]["value"], "S1");
    assert_eq!(v["diagnostics"][0]["severity"], "WARNING");
}

#[test]
fn a_clean_repository_gives_empty_documents_and_json_is_refused() {
    let r = common::repo();
    let (code, t) = r.text(&["check", "--format", "gitlab"]);
    assert_eq!(code, 0, "{t}");
    assert_eq!(serde_json::from_str::<Value>(&t).unwrap(), serde_json::json!([]));
    let (code, v) = r.json(&["check", "--format", "sarif"]);
    assert_eq!(code, 2, "{v}");
}

#[test]
fn the_pre_commit_snippet_uses_the_current_language_name() {
    let r = common::repo();
    let (_, t) = r.text(&["hook", "print", "--for", "pre-commit"]);
    assert!(t.contains("language: unsupported") && !t.contains("language: system"), "{t}");
    let manifest =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.pre-commit-hooks.yaml")).unwrap();
    assert!(manifest.contains("id: spec-check") && manifest.contains("minimum_pre_commit_version"));
}
