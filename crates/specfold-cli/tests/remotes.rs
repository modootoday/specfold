//! Documents in other repositories named `<remote>:<id>` through `[remotes.<name>]`.

mod common;

#[test]
fn remote_references_resolve_read_only_and_absent_remotes_are_skipped() {
    let shared = common::repo();
    let decision = shared.new_doc("guide", "services/shop", "Shared setup");
    let app = common::repo();
    let cfg = app.read("spec.toml");
    let dir = shared.path().to_string_lossy().to_string();
    app.write(
        "spec.toml",
        &format!("{cfg}[remotes.org]\npath = \"{dir}\"\n[remotes.ci]\npath = \"/nonexistent/shared\"\n"),
    );
    let me = app.new_doc("guide", "services/shop", "Uses the shared setup");
    let p = app.json(&["resolve", &me]).1["path"].as_str().unwrap().to_string();
    let ok = format!("org:{decision}");
    let set = |rel: &str| {
        let (code, v) = app.json(&["set", &me, rel]);
        assert_eq!(code, 0, "{v}");
    };
    set(&format!("rel.depends_on+={ok}"));
    set("rel.depends_on+=ci:guide:shop/anything");
    app.write(&p, &(app.read(&p) + &format!("\nSee [[{ok}]] and [[ci:adr:x/y]].\n")));
    let (code, v) = app.json(&["check", "--rule", "L1", "--rule", "L3"]);
    assert_eq!((code, v["errors"].as_u64()), (0, Some(0)), "{v}");

    set("rel.depends_on+=org:guide:shop/missing");
    set(&format!("rel.evidence+={ok}"));
    app.write(&p, &(app.read(&p) + &format!("\n[[{ok}#nope]]\n")));
    let (_, v) = app.json(&["check", "--rule", "L1", "--rule", "L3"]);
    let msgs: Vec<String> =
        v["items"].as_array().unwrap().iter().map(|f| f["message"].as_str().unwrap().to_string()).collect();
    assert_eq!(v["errors"], 3, "{v}");
    assert!(msgs.iter().any(|m| m.contains("names no document in the remote org")), "{msgs:?}");
    assert!(msgs.iter().any(|m| m.contains("which rel.evidence cannot point at")), "{msgs:?}");
    assert!(msgs.iter().any(|m| m.contains("has no anchor #nope")), "{msgs:?}");

    let (code, v) = app.json(&["show", &ok]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["remote"], "org", "{v}");
    assert_eq!(v["id"], decision.as_str(), "{v}");
    let (code, _) = app.json(&["show", "ci:guide:shop/anything"]);
    assert_ne!(code, 0);
    // Writes never reach the other repository.
    let before = shared.read(shared.json(&["resolve", &decision]).1["path"].as_str().unwrap());
    assert_ne!(app.json(&["set", &ok, "status=archived"]).0, 0);
    assert_eq!(shared.read(shared.json(&["resolve", &decision]).1["path"].as_str().unwrap()), before);
}
