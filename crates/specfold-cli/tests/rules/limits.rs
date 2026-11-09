//! Size budgets read from another file (B1, `spec check --rule B1 --path`) and generated files
//! read from a manifest (G2): an unreadable source is an error, never a pass.

#[path = "../common/mod.rs"]
mod common;

use serde_json::Value;

fn with_budgets(r: &common::Repo, table: &str) {
    r.write("tool.toml", table);
    r.write("spec.toml", &(r.read("spec.toml") + "[budgets]\nfrom = \"tool.toml#limits.files\"\n"));
}

fn rules(v: &Value) -> Vec<&str> {
    v["items"].as_array().unwrap().iter().map(|f| f["rule"].as_str().unwrap()).collect()
}

#[test]
fn b1_a_path_over_its_budget_fails_with_the_measures_in_json() {
    let r = common::repo();
    with_budgets(&r, "[limits.files]\n\"AGENTS.md\" = { bytes = 20, lines = 2 }\n");
    r.write("AGENTS.md", "first line\nsecond line\nthird\n");
    let (code, v) = r.json(&["check", "--rule", "B1", "--path", "AGENTS.md"]);
    assert_eq!(code, 1, "{v}");
    assert_eq!(rules(&v), ["B1", "B1"], "{v}");
    assert!(v["items"][0]["message"].as_str().unwrap().contains("over the budget of"), "{v}");
    let m = &v["budgets"][0];
    assert_eq!((m["path"].as_str(), m["bytes"].as_u64(), m["lines"].as_u64()), (Some("AGENTS.md"), Some(29), Some(3)));
    assert_eq!(m["over"], true, "{v}");
    let l = &m["limits"][0];
    assert_eq!((l["glob"].as_str(), l["source"].as_str()), (Some("AGENTS.md"), Some("tool.toml#limits.files")), "{v}");
    assert_eq!((l["max_bytes"].as_u64(), l["max_lines"].as_u64()), (Some(20), Some(2)), "{v}");
    assert_eq!((l["over_bytes"].as_bool(), l["over_lines"].as_bool()), (Some(true), Some(true)), "{v}");
}

#[test]
fn b1_text_on_stdin_is_measured_in_place_of_the_file() {
    let r = common::repo();
    with_budgets(&r, "[[limits.files]]\npaths = [\"AGENTS.md\"]\nbytes = 20\n");
    r.write("AGENTS.md", "far too long for the budget of twenty bytes\n");
    let out = r
        .spec()
        .args(["check", "--rule", "B1", "--path", "AGENTS.md", "--stdin-path", "AGENTS.md", "--json"])
        .write_stdin("short\n")
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(out.status.code(), Some(0), "{v}");
    assert_eq!((v["budgets"][0]["bytes"].as_u64(), v["budgets"][0]["over"].as_bool()), (Some(6), Some(false)));
}

#[test]
fn b1_stdin_path_alone_implies_path_for_a_file_that_is_not_a_document() {
    let r = common::repo();
    with_budgets(&r, "[limits.files]\n\"AGENTS.md\" = { bytes = 20 }\n");
    r.write("AGENTS.md", "short\n");
    let run = |text: &str| {
        let out = r
            .spec()
            .args(["check", "--rule", "B1", "--stdin-path", "AGENTS.md", "--json"])
            .write_stdin(text.to_string())
            .output()
            .unwrap();
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        (out.status.code(), v)
    };
    let (code, v) = run("far too long for the budget of twenty bytes\n");
    assert_eq!(code, Some(1), "{v}");
    assert_eq!((v["budgets"][0]["path"].as_str(), v["budgets"][0]["over"].as_bool()), (Some("AGENTS.md"), Some(true)));
    let (code, v) = run("short\n");
    assert_eq!(code, Some(0), "{v}");
}

#[test]
fn b1_an_unreadable_budget_source_is_an_error_never_a_pass() {
    let r = common::repo();
    r.write("AGENTS.md", "x\n");
    r.write("spec.toml", &(r.read("spec.toml") + "[budgets]\nfrom = \"tool.toml#limits.files\"\n"));
    for (table, says) in [
        (None, "cannot read tool.toml"),
        (Some("[limits\n"), "not valid TOML"),
        (Some("[limits.dirs]\n"), "no key"),
        (Some("[limits.files]\n\"AGENTS.md\" = { bites = 3 }\n"), "is not { bytes"),
    ] {
        if let Some(t) = table {
            r.write("tool.toml", t);
        }
        let (code, v) = r.json(&["check", "--rule", "B1", "--path", "AGENTS.md"]);
        assert_eq!(code, 1, "{says}: {v}");
        let f = &v["items"][0];
        assert_eq!((f["rule"].as_str(), f["path"].as_str()), (Some("B1"), Some("spec.toml")), "{v}");
        assert!(f["message"].as_str().unwrap().contains(says), "{says}: {v}");
    }
}

#[test]
fn b1_a_full_run_measures_every_file_a_budget_names() {
    let r = common::repo();
    r.write("spec.toml", &(r.read("spec.toml") + "[budgets.paths]\n\"services/*/README.md\" = { lines = 1 }\n"));
    r.write("services/shop/README.md", "one\ntwo\n");
    r.write("services/blog/README.md", "one\n");
    let (code, v) = r.json(&["check", "--rule", "B1"]);
    assert_eq!(code, 1, "{v}");
    let paths: Vec<&str> = v["items"].as_array().unwrap().iter().map(|f| f["path"].as_str().unwrap()).collect();
    assert_eq!(paths, ["services/shop/README.md"], "{v}");
}

fn git_in(dir: &std::path::Path, args: &[&str]) {
    let ok = std::process::Command::new("git")
        .args(["-c", "user.email=t@t", "-c", "user.name=t", "-c", "protocol.file.allow=always"])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap()
        .status
        .success();
    assert!(ok, "git {args:?}");
}

/// A repository whose submodule `vendor/lib` holds an AGENTS.md of three lines, beside a root
/// one of three lines, with `budgets` in spec.toml.
fn with_submodule(budgets: &str) -> (common::Repo, common::Repo) {
    let lib = common::Repo::new(tempfile::tempdir().unwrap());
    git_in(lib.path(), &["init", "-q", "-b", "main"]);
    lib.write("AGENTS.md", "one\ntwo\nthree\n");
    git_in(lib.path(), &["add", "-A"]);
    git_in(lib.path(), &["commit", "-qm", "lib"]);
    let r = common::repo();
    git_in(r.path(), &["submodule", "add", "-q", lib.path().to_str().unwrap(), "vendor/lib"]);
    r.write("AGENTS.md", "one\ntwo\nthree\n");
    r.write("spec.toml", &(r.read("spec.toml") + "[budgets.paths]\n" + budgets));
    git_in(r.path(), &["add", "-A"]);
    git_in(r.path(), &["commit", "-qm", "submodule"]);
    (r, lib)
}

fn tracked_json(r: &common::Repo, args: &[&str]) -> (i32, Value) {
    let out = r.spec().env_remove("SPECFOLD_INCLUDE_UNTRACKED").args(args).arg("--json").output().unwrap();
    (out.status.code().unwrap_or(-1), serde_json::from_slice(&out.stdout).unwrap())
}

fn budget_of(f: &Value) -> &str {
    f["message"].as_str().unwrap().rsplit_once('(').map_or("", |(_, g)| g)
}

fn found(v: &Value) -> Vec<(&str, &str)> {
    v["items"].as_array().unwrap().iter().map(|f| (f["path"].as_str().unwrap(), budget_of(f))).collect()
}

#[test]
fn b1_a_full_run_leaves_a_gitlink_to_its_own_repository() {
    let (r, _lib) = with_submodule("\"**/AGENTS.md\" = { lines = 1 }\n\"vendor/**\" = { lines = 1 }\n");
    for run in [tracked_json(&r, &["check", "--rule", "B1"]), r.json(&["check", "--rule", "B1"])] {
        let (code, v) = run;
        assert_eq!(code, 1, "{v}");
        assert_eq!(found(&v), [("AGENTS.md", "**/AGENTS.md in spec.toml)")], "{v}");
    }
}

#[test]
fn b1_a_budget_whose_literal_prefix_reaches_a_gitlink_judges_inside_it() {
    let (r, _lib) = with_submodule("\"**/AGENTS.md\" = { lines = 9 }\n\"vendor/lib/AGENTS.md\" = { lines = 2 }\n");
    let (code, v) = tracked_json(&r, &["check", "--rule", "B1"]);
    assert_eq!(code, 1, "{v}");
    assert_eq!(found(&v), [("vendor/lib/AGENTS.md", "vendor/lib/AGENTS.md in spec.toml)")], "{v}");
    let (code, v) = r.json(&["check", "--rule", "B1", "--path", "vendor/lib/AGENTS.md"]);
    assert_eq!(code, 1, "{v}");
    let limits: Vec<&str> =
        v["budgets"][0]["limits"].as_array().unwrap().iter().map(|l| l["glob"].as_str().unwrap()).collect();
    assert_eq!(limits, ["vendor/lib/AGENTS.md"], "{v}");
    assert!(v.get("out_of_scope").is_none(), "{v}");
}

#[test]
fn b1_a_path_inside_a_gitlink_no_budget_names_is_out_of_scope() {
    let (r, _lib) = with_submodule("\"**/AGENTS.md\" = { lines = 1 }\n");
    let (code, v) = r.json(&["check", "--rule", "B1", "--path", "vendor/lib/AGENTS.md"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["budgets"], serde_json::json!([]), "{v}");
    assert_eq!(v["out_of_scope"], serde_json::json!([{"path": "vendor/lib/AGENTS.md", "gitlink": "vendor/lib"}]));
    let (code, text) = r.text(&["check", "--rule", "B1", "--path", "vendor/lib/AGENTS.md"]);
    assert_eq!(code, 0, "{text}");
    assert!(text.contains("inside the submodule vendor/lib and no budget names it explicitly"), "{text}");
}

#[test]
fn g2_generated_copies_skip_asset_checks_and_their_sources_must_exist() {
    let r = common::repo();
    let skill = "---\nname: x\ndescription: Does x.\n---\nSee `docs/missing.md`.\n";
    r.write(".agents/skills/x/SKILL.md", skill);
    r.write(".tool/skills/x/SKILL.md", skill);
    r.write(
        "tool.lock",
        "{\"outputs\": [{\"path\": \".tool/skills/x\", \"sources\": [\".agents/skills/x/SKILL.md\"]}, {\"path\": \"gone.json#a\", \"source\": \"src/gone.toml\"}]}",
    );
    let toml = r.read("spec.toml")
        + "[[assets]]\nkind = \"skill\"\npath = \".agents/skills\"\n[[assets]]\nkind = \"skill\"\npath = \".tool/skills\"\n"
        + "[generated]\nmanifest = \"tool.lock\"\n";
    r.write("spec.toml", &toml);
    let (_, v) = r.json(&["check", "--rule", "A3", "--rule", "G2"]);
    let found: Vec<(&str, &str)> = v["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| (f["rule"].as_str().unwrap(), f["path"].as_str().unwrap()))
        .collect();
    // Errors come first; the generated copy under .tool is not read by A3.
    assert_eq!(found, [("G2", "tool.lock"), ("A3", ".agents/skills/x/SKILL.md")], "{v}");
    r.write("tool.lock", "{\"outputs\": 3}");
    let (code, v) = r.json(&["check", "--rule", "G2"]);
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["items"][0]["path"], "spec.toml", "{v}");
}
