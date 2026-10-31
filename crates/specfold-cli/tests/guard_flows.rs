//! `spec guard`: masked terms, private lists and an empty scope.

mod common;

#[test]
fn guard_masks_terms_and_reads_a_private_list() {
    let r = common::repo();
    let private = r.path().join("../private-terms.txt");
    std::fs::write(&private, "acme-internal\n").unwrap();
    let toml = r.read("spec.toml")
        + &format!(
            "[[guards]]\nname = \"public\"\npaths = [\"libs/open/**\"]\nrange = \"all\"\ndeny_terms_file = \"{}\"\n",
            private.display()
        );
    r.write("spec.toml", &toml);
    r.write("libs/open/readme.md", "Built for ACME-Internal.\n");
    let (code, v) = r.json(&["guard"]);
    assert_eq!(code, 1, "{v}");
    let msg = v["items"][0]["message"].as_str().unwrap();
    assert!(!msg.to_lowercase().contains("acme-internal"), "{msg}");
}

#[test]
fn guards_skip_spec_toml_and_explain_an_empty_scope() {
    let r = common::repo();
    let toml = r.read("spec.toml")
        + "[[guards]]\nname = \"all\"\npaths = [\"**\"]\nrange = \"all\"\ndeny_terms = [\"acme-internal\"]\n"
        + "[[guards]]\nname = \"none\"\npaths = [\"nowhere/**\"]\nrange = \"all\"\nfail_if_empty = true\ndeny_terms = [\"x\"]\n";
    r.write("spec.toml", &toml);
    r.git(&["add", "-A"]);
    let (code, v) = r.json(&["guard", "all", "--all"]);
    assert_eq!(code, 0, "spec.toml must not trip its own guard: {v}");
    let (code, text) = r.text(&["guard", "none", "--all"]);
    assert_ne!(code, 0, "{text}");
    assert!(text.contains("fail_if_empty = false"), "{text}");
}

#[test]
fn c205_guard_range_refuses_a_single_revision() {
    let r = common::repo();
    let (code, v) = r.json(&["guard", "--range", "main"]);
    assert_eq!(code, 2, "{v}");
    assert!(v["next"].as_str().unwrap().contains("main..HEAD"), "{v}");
    assert_eq!(r.json(&["guard", "--range", "HEAD..HEAD"]).0, 0);
}
