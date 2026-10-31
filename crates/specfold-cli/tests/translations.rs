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

fn with_layout(r: &common::Repo, extra: &str) {
    let cfg = r.read("spec.toml");
    r.write("spec.toml", &format!("{cfg}{extra}"));
}

fn rules(v: &serde_json::Value, rule: &str) -> Vec<String> {
    v["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["rule"] == rule)
        .map(|f| format!("{} {}", f["severity"].as_str().unwrap_or(""), f["message"].as_str().unwrap_or("")))
        .collect()
}

#[test]
fn spec_new_translation_copies_the_source_and_records_the_pair_beside_it() {
    let r = common::repo();
    with_layout(&r, "[translations]\nlayout = \"suffix\"\nlanguages = [\"ko\"]\n");
    let src = r.new_doc("adr", "services/shop", "Use carts");
    append(&r, &src, "\nKeep carts small.\n");
    let (code, v) = r.json(&["new", "--translation", "ko", "--from", &src, "--title", "카트 사용"]);
    assert_eq!(code, 0, "{v}");
    let id = v["id"].as_str().unwrap().to_string();
    let path = v["path"].as_str().unwrap().to_string();
    assert_ne!(id, src, "a translation has its own key");
    assert!(path.ends_with("-use-carts-ko.adr.md"), "{path}");
    let text = r.read(&path);
    for want in
        ["lang: ko", "source_rev: sha256:", &format!("translation_of: [{src}]"), "status: draft", "Keep carts small."]
    {
        assert!(text.contains(want), "{want} missing in {text}");
    }
    let (_, v) = r.json(&["check", "--rule", "R1", "--rule", "R2"]);
    assert_eq!(v["warnings"], 0, "{v}");
    // Not in the configured languages, and a type that is not the source's, are refused.
    assert_ne!(r.json(&["new", "--translation", "ja", "--from", &src]).0, 0);
    assert_ne!(r.json(&["new", "guide", "--translation", "ko", "--from", &src]).0, 0);
}

#[test]
fn the_suffix_layout_pairs_by_name_and_fix_writes_the_pair_down() {
    let r = common::repo();
    with_layout(&r, "[translations]\nlayout = \"suffix\"\nlanguages = [\"ko\"]\n");
    let src = r.new_doc("guide", "services/shop", "Setup");
    let p = path_of(&r, &src).replace("setup.guide.md", "setup-ko.guide.md");
    r.write(&p, "---\nid: guide:shop/setup-ko\ntitle: 설정\nstatus: active\ncreated: 2026-01-01\nupdated: 2026-01-01\n---\n# 설정\n");
    let (_, v) = r.json(&["census", "--translations"]);
    assert_eq!(v["translations"][0]["source"], src.as_str(), "{v}");
    assert_eq!(v["translations"][0]["lang"], "ko", "{v}");
    assert_eq!(v["translations"][0]["state"], "unrecorded", "{v}");
    // The implicit pair is fine as it is; source_rev works on it.
    assert_eq!(r.json(&["set", "guide:shop/setup-ko", "--source-rev"]).0, 0);
    assert_eq!(r.json(&["check", "--rule", "R1", "--rule", "R2"]).1["warnings"], 0);
    // Writing the pair down is a preview first.
    let (_, v) = r.json(&["fix", "--translations"]);
    assert_eq!(v["fixable"], 1, "{v}");
    assert!(!r.read(&p).contains("translation_of"));
    assert_eq!(r.json(&["fix", "--translations", "--write"]).0, 0);
    let t = r.read(&p);
    assert!(t.contains(&format!("translation_of: [{src}]")) && t.contains("lang: ko"), "{t}");
    assert_eq!(r.json(&["fix", "--translations"]).1["fixable"], 0);
}

#[test]
fn r2_reports_a_translation_without_source_and_a_conflicting_pair_and_strict_makes_them_errors() {
    let r = common::repo();
    with_layout(&r, "[translations]\nlayout = \"suffix\"\nlanguages = [\"ko\"]\n");
    let a = r.new_doc("guide", "services/shop", "Setup");
    let b = r.new_doc("guide", "services/shop", "Other");
    let (code, v) = r.json(&["new", "guide", "--governs", "services/shop", "--title", "Lost", "--slug", "lost-ko"]);
    assert_eq!(code, 0, "{v}");
    let (code, v) = r.json(&["new", "--translation", "ko", "--from", &a]);
    assert_eq!(code, 0, "{v}");
    let ko = v["id"].as_str().unwrap().to_string();
    assert_eq!(
        r.json(&["set", &ko, "lang=ja", &format!("rel.translation_of-={a}"), &format!("rel.translation_of+={b}")]).0,
        0
    );
    let (code, v) = r.json(&["check", "--rule", "R2"]);
    assert_eq!(code, 0, "{v}");
    let got = rules(&v, "R2");
    assert_eq!(got.len(), 3, "{got:?}");
    assert!(got.iter().any(|m| m.contains("no source is in the matching place")), "{got:?}");
    assert!(got.iter().any(|m| m.contains("lang is 'ja'")), "{got:?}");
    assert!(got.iter().any(|m| m.contains(&format!("names {b} but the layout pairs it with {a}"))), "{got:?}");
    with_layout(&r, "strict = true\n");
    let (code, v) = r.json(&["check", "--rule", "R2"]);
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["errors"], 3, "{v}");
}

#[test]
fn the_dir_layout_pairs_the_same_slug_under_a_language_owner() {
    let r = common::repo();
    r.write("services/shop/ko/README.md", "ko\n");
    with_layout(
        &r,
        "[[owners.kinds]]\npattern = \"services/*/ko\"\nid = \"{1}-ko\"\n[translations]\nlayout = \"dir\"\nlanguages = [\"ko\"]\n",
    );
    let src = r.new_doc("guide", "services/shop", "Setup");
    let (code, v) = r.json(&["new", "--translation", "ko", "--from", &src]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["id"], "guide:shop-ko/setup", "{v}");
    assert!(v["path"].as_str().unwrap().starts_with("services/shop/ko/"), "{v}");
    // Paired by place even without frontmatter.
    let p = v["path"].as_str().unwrap().to_string();
    let t = r.read(&p);
    let stripped: String =
        t.lines().filter(|l| !l.starts_with("rel:") && !l.contains("translation_of")).collect::<Vec<_>>().join("\n");
    r.write(&p, &stripped);
    let (_, v) = r.json(&["census", "--translations"]);
    assert_eq!(v["translations"][0]["source"], src.as_str(), "{v}");
    assert_eq!(r.json(&["check", "--rule", "R2"]).1["warnings"], 0);
}

#[test]
fn set_refuses_a_bad_language_tag_or_source_rev() {
    let (r, _src, ko) = pair();
    assert_ne!(r.json(&["set", &ko, "lang=korean language"]).0, 0);
    assert_ne!(r.json(&["set", &ko, "source_rev=abc"]).0, 0);
    assert_eq!(r.json(&["set", &ko, "lang=pt-BR"]).0, 0);
    assert_eq!(r.json(&["set", &ko, "source_rev=sha256:0123456789ab"]).0, 0);
}

#[test]
fn a_translation_of_a_translation_is_an_error_and_not_a_census_source() {
    let (r, src, ko) = pair();
    let ja = r.new_doc("guide", "services/shop", "Setup in Japanese");
    let rel = format!("rel.translation_of+={ko}");
    assert_eq!(r.json(&["set", &ja, "lang=ja", &rel]).0, 0);
    let (code, v) = r.json(&["check", "--rule", "R1"]);
    assert_eq!(code, 1, "{v}");
    assert!(messages(&v).iter().any(|m| m.contains("is itself a translation") && m.contains(&src)), "{v}");
    let (_, v) = r.json(&["census", "--translations"]);
    assert_eq!(v["sources"], 1, "{v}");
}

#[test]
fn two_translations_into_one_language_warn() {
    let (r, src, _ko) = pair();
    let ko2 = r.new_doc("guide", "services/shop", "Setup in Korean again");
    let rel = format!("rel.translation_of+={src}");
    assert_eq!(r.json(&["set", &ko2, "lang=ko", &rel]).0, 0);
    let (_, v) = r.json(&["check", "--rule", "R1"]);
    assert!(messages(&v).iter().any(|m| m.contains("another 'ko' translation")), "{v}");
}

#[test]
fn schema_lists_the_translation_keys() {
    let r = common::repo();
    let (_, v) = r.json(&["schema", "guide"]);
    let p = &v["frontmatter"]["properties"];
    assert!(p["lang"].is_object() && p["source_rev"].is_object(), "{v}");
    assert!(p["rel"]["properties"]["translation_of"].is_object(), "{v}");
}

#[test]
fn new_translation_lowercases_the_slug_tag_and_refuses_a_translation_as_source() {
    let r = common::repo();
    with_layout(&r, "[translations]\nlayout = \"suffix\"\nlanguages = [\"ko-KR\", \"ja\"]\n");
    let src = r.new_doc("guide", "services/shop", "Install the CLI");
    let (code, v) = r.json(&["new", "--translation", "ko-KR", "--from", &src]);
    assert_eq!(code, 0, "{v}");
    let ko = v["id"].as_str().unwrap().to_string();
    assert!(ko.ends_with("install-the-cli-ko-kr"), "{ko}");
    assert!(r.read(v["path"].as_str().unwrap()).contains("lang: ko-KR"));
    let (_, v) = r.json(&["check"]);
    assert_eq!(v["errors"], 0, "{v}");
    assert!(rules(&v, "R2").is_empty() && rules(&v, "S3").is_empty(), "{v}");
    let (code, v) = r.json(&["new", "--translation", "ja", "--from", &ko]);
    assert_ne!(code, 0, "{v}");
    assert!(v.to_string().contains(&format!("--from {src}")), "{v}");
}

#[test]
fn new_and_set_accept_write_and_errors_keep_their_own_command_as_next() {
    let r = common::repo();
    let src = r.new_doc("guide", "services/shop", "Install");
    let (code, v) = r.json(&["new", "--translation", "ja", "--from", &src, "--write"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(r.json(&["set", &src, "tags+=x", "--write"]).0, 0);
    let ja = r.json(&["find", "install-ja"]).1["items"][0]["id"].as_str().unwrap().to_string();
    let (code, v) = r.json(&["new", "--translation", "ko", "--from", &ja]);
    assert_ne!(code, 0);
    assert_eq!(v["next"], format!("spec new --translation ko --from {src}"), "{v}");
    let (_, v) = r.json(&["new", "--translation", "ko", "--from", &src, "--bogus"]);
    assert!(!v["next"].as_str().unwrap().contains("--title"), "{v}");
}

#[test]
fn c212_census_translations_and_graph_stream_their_rows() {
    let (r, _src, _ko) = pair();
    let lines = |args: &[&str]| -> Vec<serde_json::Value> {
        let out = r.spec().args(args).output().unwrap();
        String::from_utf8_lossy(&out.stdout).lines().map(|l| serde_json::from_str(l).unwrap()).collect()
    };
    let rows = lines(&["census", "--translations", "--lang", "ja", "--jsonl"]);
    assert_eq!(rows.len(), 3, "{rows:?}");
    assert_eq!(rows[0]["state"], "unrecorded", "{rows:?}");
    assert_eq!(rows[1]["state"], "missing", "{rows:?}");
    assert_eq!(rows[2]["total"], 2, "{rows:?}");
    let (_, v) = r.json(&["census", "--translations", "--lang", "ja", "--limit", "1"]);
    assert_eq!(v["items"].as_array().unwrap().len(), 1, "{v}");
    assert_eq!(v["more"], 1, "{v}");
    let rows = lines(&["graph", "--jsonl"]);
    let nodes = rows.iter().filter(|x| x["row"] == "node").count();
    assert!(nodes >= 2, "{rows:?}");
    assert!(rows.iter().any(|x| x["row"] == "edge"), "{rows:?}");
}
