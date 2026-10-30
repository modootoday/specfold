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

fn linked() -> (common::Repo, common::Repo, String) {
    let shared = common::repo();
    let doc = shared.new_doc("guide", "services/shop", "Shared setup");
    let app = common::repo();
    let cfg = app.read("spec.toml");
    let dir = shared.path().to_string_lossy().to_string();
    app.write(
        "spec.toml",
        &format!(
            "{cfg}[cites]\npaths = [\"README.md\"]\n[remotes.org]\npath = \"{dir}\"\n[remotes.ci]\npath = \"/nonexistent/shared\"\n"
        ),
    );
    (shared, app, doc)
}

#[test]
fn cites_outside_documents_are_checked_against_remotes() {
    let (_shared, app, doc) = linked();
    app.write("README.md", &format!("See [[org:{doc}]] and [[ci:guide:a/b]].\n"));
    let (code, v) = app.json(&["check", "--rule", "L6"]);
    assert_eq!((code, v["errors"].as_u64()), (0, Some(0)), "{v}");
    app.write("README.md", "See [[org:guide:shop/missing]].\n");
    let (code, v) = app.json(&["check", "--rule", "L6"]);
    assert_eq!(code, 1, "{v}");
    assert!(v["items"][0]["message"].as_str().unwrap().contains("names no document in the remote org"), "{v}");
}

#[test]
fn a_translation_of_a_remote_source_records_and_tracks_its_revision() {
    let (shared, app, doc) = linked();
    let ko = app.new_doc("guide", "services/shop", "Setup in Korean");
    let rel = format!("rel.translation_of+=org:{doc}");
    assert_eq!(app.json(&["set", &ko, "lang=ko", &rel]).0, 0);
    let (_, v) = app.json(&["check", "--rule", "R1"]);
    assert_eq!(v["warnings"], 1, "{v}");
    assert_eq!(app.json(&["set", &ko, "--source-rev"]).0, 0);
    assert_eq!(app.json(&["check", "--rule", "R1"]).1["warnings"], 0);
    let p = shared.json(&["resolve", &doc]).1["path"].as_str().unwrap().to_string();
    shared.write(&p, &(shared.read(&p) + "\nA new step.\n"));
    let (_, v) = app.json(&["check", "--rule", "R1"]);
    assert!(v["items"][0]["message"].as_str().unwrap().contains("changed since source_rev"), "{v}");
}

#[test]
fn doctor_lists_remotes_and_whether_they_are_present() {
    let (_shared, app, _doc) = linked();
    let (_, v) = app.json(&["doctor"]);
    let r = v["remotes"].as_array().unwrap();
    assert_eq!(r.len(), 2, "{v}");
    assert_eq!(r.iter().find(|x| x["name"] == "org").unwrap()["present"], true, "{v}");
    assert_eq!(r.iter().find(|x| x["name"] == "ci").unwrap()["present"], false, "{v}");
}
