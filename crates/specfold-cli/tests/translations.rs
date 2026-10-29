//! Translated documents: rel.translation_of, source_rev, rule R1 and `census --translations`.

mod common;

fn path_of(r: &common::Repo, id: &str) -> String {
    r.json(&["resolve", id]).1["path"].as_str().unwrap().to_string()
}

fn append(r: &common::Repo, id: &str, text: &str) {
    let p = path_of(r, id);
    r.write(&p, &(r.read(&p) + text));
}

fn messages(v: &serde_json::Value) -> Vec<String> {
    v["items"].as_array().unwrap().iter().map(|f| f["message"].as_str().unwrap_or("").to_string()).collect()
}

fn pair() -> (common::Repo, String, String) {
    let r = common::repo();
    let src = r.new_doc("guide", "services/shop", "Setup");
    let ko = r.new_doc("guide", "services/shop", "Setup in Korean");
    let rel = format!("rel.translation_of+={src}");
    assert_eq!(r.json(&["set", &ko, "lang=ko", &rel]).0, 0);
    (r, src, ko)
}

#[test]
fn a_translation_records_its_source_and_warns_when_the_source_changes() {
    let (r, src, ko) = pair();
    let (_, v) = r.json(&["check", "--rule", "R1"]);
    assert_eq!(v["warnings"], 1, "{v}");
    assert!(messages(&v)[0].contains("records no source_rev"), "{v}");
    assert_eq!(v["items"][0]["fix"]["cmd"].as_str().map(|s| s.contains("--source-rev")), Some(true), "{v}");

    let (code, v) = r.json(&["set", &ko, "--source-rev"]);
    assert_eq!(code, 0, "{v}");
    assert!(r.read(&path_of(&r, &ko)).contains("source_rev: sha256:"));
    assert_eq!(r.json(&["check", "--rule", "R1", "--rule", "S1", "--rule", "L3"]).1["warnings"], 0);

    // Whitespace alone is not a change.
    append(&r, &src, "\n\n");
    assert_eq!(r.json(&["check", "--rule", "R1"]).1["warnings"], 0);

    append(&r, &src, "\nA new step.\n");
    let (_, v) = r.json(&["check", "--rule", "R1"]);
    assert_eq!(v["warnings"], 1, "{v}");
    assert!(messages(&v)[0].contains("changed since source_rev"), "{v}");

    assert_eq!(r.json(&["set", &ko, "--source-rev"]).0, 0);
    assert_eq!(r.json(&["check", "--rule", "R1"]).1["warnings"], 0);
}

#[test]
fn census_lists_translation_state_and_missing_languages_with_an_exit_code() {
    let (r, src, ko) = pair();
    let (code, v) = r.json(&["census", "--translations"]);
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["translations"][0]["state"], "unrecorded", "{v}");
    assert_eq!(r.json(&["set", &ko, "--source-rev"]).0, 0);
    let (code, v) = r.json(&["census", "--translations"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["translations"][0]["state"], "current", "{v}");
    let (code, v) = r.json(&["census", "--translations", "--lang", "ja"]);
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["missing"]["ja"][0], src.as_str(), "{v}");
    // Missing translations fail by default; --fail-on stale or none report them only.
    assert_eq!(r.json(&["census", "--translations", "--lang", "ja", "--fail-on", "stale"]).0, 0);
    assert_eq!(r.json(&["census", "--translations", "--lang", "ja", "--fail-on", "none"]).0, 0);
    append(&r, &src, "\nA new paragraph.\n");
    assert_eq!(r.json(&["census", "--translations", "--fail-on", "stale"]).0, 1);
    assert_eq!(r.json(&["census", "--translations", "--fail-on", "none"]).0, 0);
}

#[test]
fn a_removed_source_is_a_broken_relation_not_a_lag() {
    let (r, src, ko) = pair();
    assert_eq!(r.json(&["set", &ko, "--source-rev"]).0, 0);
    std::fs::remove_file(r.path().join(path_of(&r, &src))).unwrap();
    let (code, v) = r.json(&["check", "--rule", "L3"]);
    assert_eq!(code, 1, "{v}");
    assert_eq!(r.json(&["check", "--rule", "R1"]).1["warnings"], 0);
    let (_, v) = r.json(&["census", "--translations"]);
    assert_eq!(v["translations"][0]["state"], "no_source", "{v}");
}

#[test]
fn source_rev_needs_a_translation_of() {
    let r = common::repo();
    let id = r.new_doc("guide", "services/shop", "Alone");
    let (code, v) = r.json(&["set", &id, "--source-rev"]);
    assert_ne!(code, 0, "{v}");
}

#[test]
fn a_behind_translation_names_the_sections_that_changed_when_git_has_its_revision() {
    let (r, src, ko) = pair();
    let p = path_of(&r, &src);
    let body = r.read(&p);
    r.write(&p, &format!("{body}\n## Install\n\nRun the installer.\n\n## Limits\n\nNone yet.\n"));
    assert_eq!(r.json(&["set", &ko, "--source-rev"]).0, 0);
    r.commit("source and translation");
    let now = r.read(&p).replace("Run the installer.", "Run the installer twice.");
    r.write(&p, &format!("{now}\n## Support\n\nAsk the team.\n"));
    let (_, v) = r.json(&["check", "--rule", "R1"]);
    let m = &messages(&v)[0];
    assert!(m.contains("in Install (1 paragraph changed), Support (new)"), "{m}");
    assert!(!m.contains("Limits"), "{m}");
    let (_, v) = r.json(&["census", "--translations"]);
    assert_eq!(v["translations"][0]["sections"][0]["section"], "Install", "{v}");
    assert_eq!(v["translations"][0]["sections"][1]["change"], "added", "{v}");
    // Without the revision in git, the warning still says the source changed.
    let (_, v) = r.json(&["set", &ko, "source_rev=sha256:000000000000"]);
    assert_eq!(v["ok"], true, "{v}");
    let (_, v) = r.json(&["check", "--rule", "R1"]);
    let m = &messages(&v)[0];
    assert!(m.contains("changed since source_rev sha256:000000000000: update"), "{m}");
}
