//! Status moves and list values through `spec set`: required fields, JSON lists, and successors
//! that take effect after `spec new --from`.

#[path = "../common/mod.rs"]
mod common;

use serde_json::json;

fn status(r: &common::Repo, id: &str) -> String {
    r.json(&["show", id]).1["frontmatter"]["status"].as_str().unwrap_or_default().to_string()
}

#[test]
fn f3_set_reads_a_json_list_without_keeping_its_quotes() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Supported versions");
    let cases: &[(&str, serde_json::Value)] = &[
        (r#"tags=["7.3"]"#, json!(["7.3"])),
        (r#"tags=["7.3", "a, b"]"#, json!(["7.3", "a, b"])),
        ("tags=[7.3, 10.20]", json!(["7.3", "10.20"])),
        ("tags=[x, y]", json!(["x", "y"])),
    ];
    for (assign, want) in cases {
        let (code, v) = r.json(&["set", &id, assign, "--write"]);
        assert_eq!(code, 0, "{assign}: {v}");
        assert_eq!(r.json(&["show", &id]).1["frontmatter"]["tags"], *want, "{assign}");
    }
    let path = common::doc_path(&r, &id);
    assert!(!r.read(&path).contains(r#"\""#), "no escaped quotes are written: {}", r.read(&path));
}

#[test]
fn f4_set_status_refuses_missing_required_fields_and_names_the_command() {
    let r = common::repo();
    let toml = r.read("spec.toml") + "[types.adr]\nrequire_when = { active = [\"decision_makers\", \"consulted\"] }\n";
    r.write("spec.toml", &toml);
    let id = r.new_doc("adr", "services/shop", "Use one queue");
    let path = common::doc_path(&r, &id);
    let before = r.read(&path);
    for write in [false, true] {
        let mut a = vec!["set", id.as_str(), "status=active", "consulted=lee"];
        if write {
            a.push("--write");
        }
        let (code, v) = r.json(&a);
        assert_eq!(code, 2, "{v}");
        let error = v["error"].as_str().unwrap_or_default();
        assert!(error.contains("requires decision_makers") && !error.contains("consulted,"), "{v}");
        assert_eq!(
            v["next"].as_str().unwrap_or_default(),
            format!("spec set {id} status=active consulted=lee decision_makers=… --write"),
            "{v}"
        );
        assert_eq!(r.read(&path), before, "nothing written");
    }
    let (code, v) = r.json(&["set", &id, "status=active", "consulted=lee", "decision_makers=kim", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(r.json(&["check", "--rule", "S6"]).1["errors"], 0);
}

#[test]
fn f5_new_from_leaves_the_old_document_until_the_successor_is_active() {
    let r = common::repo();
    let old = r.new_doc("adr", "services/shop", "Use files");
    assert_eq!(r.json(&["set", &old, "status=active", "--write"]).0, 0);
    let (code, v) = r.json(&["new", "adr", "--from", &old, "--title", "Use a database", "--write"]);
    assert_eq!(code, 0, "{v}");
    let new = v["id"].as_str().unwrap().to_string();
    let note = v["supersede_note"].as_str().unwrap_or_default();
    assert!(note.contains(&format!("spec set {new} status=active --write")), "{v}");
    assert_eq!(status(&r, &old), "active", "a draft successor has not taken effect");
    assert_eq!(r.json(&["check"]).0, 0, "a pending successor is no T1 finding: {}", r.json(&["check"]).1);
    let (code, v) = r.json(&["set", &new, "status=active", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(status(&r, &old), "superseded", "the successor took effect in the same write");
    assert_eq!(r.json(&["check"]).0, 0, "{}", r.json(&["check"]).1);
}

#[test]
fn f5_a_rejected_successor_leaves_the_old_document_in_force() {
    let r = common::repo();
    let old = r.new_doc("adr", "services/shop", "Use files");
    assert_eq!(r.json(&["set", &old, "status=active", "--write"]).0, 0);
    let (_, v) = r.json(&["new", "adr", "--from", &old, "--title", "Use a database", "--write"]);
    let new = v["id"].as_str().unwrap().to_string();
    assert_eq!(r.json(&["set", &new, "status=rejected", "--write"]).0, 0);
    assert_eq!(status(&r, &old), "active");
    assert_eq!(r.json(&["check", "--rule", "T1"]).1["errors"], 0);
}

#[test]
fn f5_a_type_that_starts_in_force_supersedes_at_once() {
    let r = common::repo();
    let old = r.new_doc("research", "services/shop", "Queue benchmark");
    let (code, v) = r.json(&["new", "research", "--from", &old, "--title", "Queue benchmark again", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert!(v.get("supersede_note").is_none(), "{v}");
    assert_eq!(status(&r, &old), "superseded");
    assert_eq!(r.json(&["check", "--rule", "T1"]).1["errors"], 0);
}

#[test]
fn f5_t1_still_flags_an_active_successor_whose_old_document_is_not_superseded() {
    let r = common::repo();
    let old = r.new_doc("adr", "services/shop", "Use files");
    assert_eq!(r.json(&["set", &old, "status=active", "--write"]).0, 0);
    let (_, v) = r.json(&["new", "adr", "--from", &old, "--title", "Use a database", "--write"]);
    let new = v["id"].as_str().unwrap().to_string();
    let path = common::doc_path(&r, &new);
    r.write(&path, &r.read(&path).replace("status: draft", "status: active"));
    let (_, v) = r.json(&["check", "--rule", "T1"]);
    assert_eq!(v["errors"], 1, "{v}");
    assert!(v.to_string().contains(&format!("spec set {old} status=superseded --write")), "{v}");
}
