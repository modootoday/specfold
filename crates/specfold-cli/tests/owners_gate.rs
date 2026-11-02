//! D1: the push gate of a repository holding several projects (`check --owners`, `hook install`).

mod common;

fn break_id(r: &common::Repo, id: &str) {
    let path = r.json(&["resolve", id]).1["path"].as_str().unwrap().to_string();
    let text = r.read(&path).replace(&format!("id: {id}"), &format!("id: {id}-wrong"));
    r.write(&path, &text);
}

#[test]
fn owners_fails_only_on_touched_owners() {
    let r = common::repo();
    let shop = r.new_doc("sot", "services/shop", "Cart");
    let blog = r.new_doc("sot", "services/blog", "Posts");
    break_id(&r, &blog);
    r.commit("blog broken");
    r.write("services/shop/src/extra.rs", "fn x() {}\n");
    let (code, v) = r.json(&["check", "--changed", "HEAD", "--owners", "--rule", "S2"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["other_owners"]["blog"]["errors"], 1, "{v}");
    let (_, t) = r.text(&["check", "--changed", "HEAD", "--owners", "--rule", "S2"]);
    assert!(t.contains("blog (services/blog): 1 error(s) in an owner this change does not touch"), "{t}");
    break_id(&r, &shop);
    let (code, v) = r.json(&["check", "--changed", "HEAD", "--owners", "--rule", "S2"]);
    assert_eq!(code, 1, "{v}");
    // Without --owners nothing changes.
    let (_, v) = r.json(&["check", "--changed", "HEAD", "--rule", "S2"]);
    assert!(v.get("other_owners").is_none(), "{v}");
}

#[test]
fn hook_install_previews_writes_and_refuses_foreign_hooks() {
    let r = common::repo();
    let (code, t) = r.text(&["hook", "install"]);
    assert_eq!(code, 0);
    assert!(t.contains("spec check --owners --changed") && t.contains("NOT WRITTEN"), "{t}");
    assert!(!r.path().join(".git/hooks/pre-push").exists());
    let (code, v) = r.json(&["hook", "install", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert!(r.read(".git/hooks/pre-push").contains("--owners"));
    let (code, t) = r.text(&["hook", "install", "--write"]);
    assert_eq!(code, 0);
    assert!(t.contains("already installed"), "{t}");
    r.write(".git/hooks/pre-push", "#!/bin/sh\nother-tool\n");
    let (code, _) = r.json(&["hook", "install", "--write"]);
    assert_eq!(code, 4);
    assert_eq!(r.read(".git/hooks/pre-push"), "#!/bin/sh\nother-tool\n");
}
