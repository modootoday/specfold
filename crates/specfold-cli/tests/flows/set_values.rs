//! `spec set` writes values in the shape the field holds and leaves the rest of the frontmatter
//! as it was.

#[path = "../common/mod.rs"]
mod common;

/// A sot and the repository path of its file.
fn sot(r: &common::Repo) -> (String, String) {
    let id = r.new_doc("sot", "services/shop", "Cart");
    let path = common::doc_path(r, &id);
    (id, path)
}

#[test]
fn set_writes_a_numeric_field_as_a_yaml_number() {
    let r = common::repo();
    let (id, path) = sot(&r);
    let (code, v) = r.json(&["set", &id, "verify_timeout_ms=180000", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert!(r.read(&path).contains("\nverify_timeout_ms: 180000\n"), "{}", r.read(&path));
    let (_, d) = r.json(&["show", &id]);
    assert_eq!(d["frontmatter"]["verify_timeout_ms"], 180000, "{d}");
}

#[test]
fn set_refuses_a_numeric_field_that_is_not_a_number() {
    let r = common::repo();
    let (id, path) = sot(&r);
    let before = r.read(&path);
    let (code, v) = r.json(&["set", &id, "verify_timeout_ms=3m", "--write"]);
    assert_eq!(code, 2, "{v}");
    assert!(v["error"].as_str().unwrap().contains("verify_timeout_ms takes a whole number"), "{v}");
    assert_eq!(r.read(&path), before);
}

#[test]
fn set_infers_a_number_for_an_unknown_key_unless_it_holds_text() {
    let r = common::repo();
    let (id, path) = sot(&r);
    r.write(&path, &r.read(&path).replacen("---\n", "---\nticket: AB\n", 1));
    let (code, v) = r.json(&["set", &id, "retries=3", "ticket=12", "build=007", "--write"]);
    assert_eq!(code, 0, "{v}");
    let text = r.read(&path);
    assert!(text.contains("\nretries: 3\n"), "a new key holding a whole number is one: {text}");
    assert!(text.contains("ticket: \"12\"\n"), "a key that holds text keeps text: {text}");
    assert!(text.contains("\nbuild: \"007\"\n"), "a spelling that is not the number's own stays text: {text}");
    let (code, v) = r.json(&["set", &id, "retries=4", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert!(r.read(&path).contains("\nretries: 4\n"), "{}", r.read(&path));
}
