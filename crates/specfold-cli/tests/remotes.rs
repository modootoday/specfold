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
        let (code, v) = app.json(&["set", &me, rel, "--write"]);
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
    assert_ne!(app.json(&["set", &ok, "status=archived", "--write"]).0, 0);
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
    assert_eq!(app.json(&["set", &ko, "lang=ko", &rel, "--write"]).0, 0);
    let (_, v) = app.json(&["check", "--rule", "R1"]);
    assert_eq!(v["warnings"], 1, "{v}");
    assert_eq!(app.json(&["set", &ko, "--source-rev", "--write"]).0, 0);
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

#[test]
fn a_pinned_remote_document_warns_when_it_changes() {
    let (shared, app, doc) = linked();
    let me = app.new_doc("guide", "services/shop", "Relies on the shared setup");
    let pin = format!("org:{doc}");
    let (code, v) = app.json(&["set", &me, "--pin", &pin, "--write"]);
    assert_eq!(code, 0, "{v}");
    let (_, v) = app.json(&["check"]);
    assert_eq!(v["errors"], 0, "{v}");
    assert!(app.json(&["check", "--rule", "S1"]).1["items"].as_array().unwrap().is_empty());
    assert!(app.json(&["check", "--rule", "L7"]).1["items"].as_array().unwrap().is_empty());
    let p = shared.json(&["resolve", &doc]).1["path"].as_str().unwrap().to_string();
    shared.write(&p, &(shared.read(&p) + "\nA new step.\n"));
    let (_, v) = app.json(&["check", "--rule", "L7"]);
    assert!(v["items"][0]["message"].as_str().unwrap().contains("changed since the pinned"), "{v}");
    assert_eq!(app.json(&["set", &me, "--pin", &pin, "--write"]).0, 0);
    assert!(app.json(&["check", "--rule", "L7"]).1["items"].as_array().unwrap().is_empty());
    assert_ne!(app.json(&["set", &me, "--pin", "ci:guide:a/b", "--write"]).0, 0);
    let mp = app.json(&["resolve", &me]).1["path"].as_str().unwrap().to_string();
    let text = app.read(&mp).replacen("pins:\n", "pins:\n  org:guide:shop/missing: sha256:0123456789ab\n", 1);
    app.write(&mp, &text);
    let (code, v) = app.json(&["check", "--rule", "L7"]);
    assert_eq!(code, 1, "{v}");
}

#[test]
fn consumers_of_a_shared_document_are_listed_from_either_side() {
    let (shared, app, doc) = linked();
    let me = app.new_doc("guide", "services/shop", "Relies on the shared setup");
    assert_eq!(app.json(&["set", &me, "--pin", &format!("org:{doc}"), "--write"]).0, 0);
    let (code, v) = app.json(&["graph", "consumers", &format!("org:{doc}")]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["items"][0]["repo"], ".", "{v}");
    assert_eq!(v["items"][0]["id"], me.as_str(), "{v}");
    assert_eq!(v["items"][0]["via"], "pins", "{v}");
    let cfg = shared.read("spec.toml");
    shared.write("spec.toml", &format!("{cfg}[remotes.apps]\npath = \"{}\"\n", app.path().to_string_lossy()));
    let (code, v) = shared.json(&["graph", "consumers", &doc]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["items"][0]["repo"], "apps", "{v}");
    assert_eq!(v["items"].as_array().unwrap().len(), 1, "{v}");
    assert_ne!(app.json(&["graph", "consumers", "ci:guide:a/b"]).0, 0);
    let (_, t) = app.text(&["graph", "consumers", &format!("org:{doc}")]);
    assert!(t.contains("repo") && t.contains("via") && t.contains("repo '.' is this repository"), "{t}");
    assert!(t.contains("1 consumer of"), "{t}");
}

#[test]
fn no_consumers_says_so_and_names_where_it_searched() {
    let r = common::repo();
    let doc = r.new_doc("guide", "services/shop", "Nobody uses this");
    let (code, t) = r.text(&["graph", "consumers", &doc]);
    assert_eq!(code, 0, "{t}");
    assert!(t.contains("0 consumers of") && t.contains("no [remotes] in spec.toml"), "{t}");
    assert!(r.json(&["graph", "consumers", &doc]).1["searched"].as_str().unwrap().contains("this repository"));
}

#[test]
fn pins_are_removed_with_unpin_and_pin_errors_name_the_case() {
    let (_shared, app, doc) = linked();
    let me = app.new_doc("guide", "services/shop", "Relies on the shared setup");
    let local = app.new_doc("guide", "services/shop", "Local one");
    let msg = |args: &[&str]| {
        let (code, v) = app.json(args);
        assert_ne!(code, 0, "{v}");
        v.to_string()
    };
    assert!(msg(&["set", &me, "--pin", "zzz:guide:a/b", "--write"]).contains("'zzz' is not a configured remote"));
    assert!(msg(&["set", &me, "--pin", &local, "--write"]).contains("a document in this repository"));
    assert!(msg(&["set", &me, "--pin", "org:guide:shop/none", "--write"]).contains("the remote org has no document"));
    assert!(msg(&["set", &me, "--pin", "ci:guide:a/b", "--write"]).contains("not on disk"));
    let pin = format!("org:{doc}");
    let (_, v) = app.json(&["set", &me, "--pin", &pin, "--write"]);
    assert!(v["next"].as_str().unwrap_or_default().contains("--rule L7"), "{v}");
    assert!(msg(&["set", &me, "--unpin", "org:guide:shop/other", "--write"]).contains("has no pin"));
    assert_eq!(app.json(&["set", &me, "--unpin", &pin, "--write"]).0, 0);
    let p = app.json(&["resolve", &me]).1["path"].as_str().unwrap().to_string();
    assert!(!app.read(&p).contains("pins"), "{}", app.read(&p));
}

#[test]
fn a_translation_cannot_start_from_a_remote_source() {
    let (_shared, app, doc) = linked();
    let (code, v) = app.json(&["new", "--translation", "ko", "--from", &format!("org:{doc}"), "--write"]);
    assert_ne!(code, 0, "{v}");
    assert!(v.to_string().contains("never copies from"), "{v}");
}

#[test]
fn governs_into_another_project_is_external_for_p1_and_l5() {
    let app = common::repo();
    let cfg = app.read("spec.toml");
    app.write("spec.toml", &format!("{cfg}[remotes.child]\npath = \"../nowhere\"\npaths = [\"vendor/child\"]\n"));
    let me = app.new_doc("sot", "services/shop", "Uses child code");
    for g in ["rel.governs+=vendor/child/src", "rel.governs+=child:src/lib.rs"] {
        let (code, v) = app.json(&["set", &me, g, "--write"]);
        assert_eq!(code, 0, "{v}");
    }
    let (_, v) = app.json(&["check", "--rule", "P1", "--rule", "L5"]);
    assert_eq!(v["errors"], 0, "{v}");
    assert_eq!(v["warnings"], 0, "{v}");
}

#[test]
fn a_submodule_with_its_own_spec_toml_is_a_read_only_remote_when_asked() {
    let child = common::repo();
    let rule = child.new_doc("sot", "services/shop/src", "Child rule");
    child.write("services/shop/src/a.rs", "fn a() {}\n");
    child.commit("child");
    let app = common::repo();
    let url = child.path().to_str().unwrap();
    app.git(&["-c", "protocol.file.allow=always", "submodule", "add", "-q", url, "vendor/child"]);
    let me = app.new_doc("guide", "services/shop", "Uses child");
    let p = app.json(&["resolve", &me]).1["path"].as_str().unwrap().to_string();
    app.write(&p, &(app.read(&p) + &format!("\nSee [[child:{rule}]].\n")));
    assert_eq!(app.json(&["check", "--rule", "L1"]).1["errors"], 1, "off by default");
    app.write("spec.toml", &app.read("spec.toml").replace("schema = 1\n", "schema = 1\nsubmodule_remotes = true\n"));
    let (code, v) = app.json(&["check", "--rule", "L1"]);
    assert_eq!((code, v["errors"].as_u64()), (0, Some(0)), "{v}");
    let (code, t) = app.text(&["context", "vendor/child/services/shop/src/a.rs"]);
    assert_eq!(code, 0, "{t}");
    assert!(t.contains("owner child:shop, read-only remote") && t.contains(&format!("child:{rule}")), "{t}");
}

#[test]
fn another_repository_is_read_with_show_in_and_a_retired_remote_target_warns_when_asked() {
    let shared = common::repo();
    let old = shared.new_doc("guide", "services/shop", "Old setup");
    let app = common::repo();
    let dir = shared.path().to_string_lossy().to_string();
    let (code, t) = app.text(&["show", &old, "--in", &dir]);
    assert!(code == 0 && t.contains("Old setup"), "{t}");
    app.write("spec.toml", &format!("{}[remotes.org]\npath = \"{dir}\"\n", app.read("spec.toml")));
    let me = app.new_doc("guide", "services/shop", "Uses it");
    assert_eq!(app.json(&["set", &me, &format!("rel.depends_on+=org:{old}"), "--write"]).0, 0);
    let new = shared.new_doc("guide", "services/shop", "New setup");
    assert_eq!(shared.json(&["set", &new, &format!("rel.supersedes+={old}"), "--write"]).0, 0);
    assert_eq!(shared.json(&["set", &old, "status=superseded", "--write"]).0, 0, "{}", shared.json(&["show", &old]).1);
    assert_eq!(app.json(&["check", "--rule", "L3"]).1["warnings"], 0, "off by default");
    app.write("spec.toml", &format!("{}[check]\nremote_status = true\n", app.read("spec.toml")));
    let (code, t) = app.text(&["check", "--rule", "L3"]);
    assert_eq!(code, 0, "{t}");
    assert!(t.contains("is superseded in its repository") && t.contains(&format!("rel.depends_on+=org:{new}")), "{t}");
}

#[test]
fn a_public_repository_mapped_to_a_private_root_resolves_ids_and_writes_nothing_in_its_tree() {
    let private = common::repo();
    let id = private.new_doc("guide", "services/shop", "Private note");
    let public = tempfile::tempdir().unwrap();
    let pubdir = public.path().join("lib");
    std::fs::create_dir_all(&pubdir).unwrap();
    assert!(std::process::Command::new("git").args(["init", "-q"]).current_dir(&pubdir).status().unwrap().success());
    let roots = public.path().join("roots.toml");
    let run = |args: &[&str]| {
        let out = private.spec().current_dir(&pubdir).env("SPECFOLD_ROOTS", &roots).args(args).output().unwrap();
        (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned())
    };
    assert_ne!(run(&["show", &id]).0, 0, "unmapped: the public repository has no such document");
    let text = format!("[[root]]\nrepo = \"{}\"\nspec = \"{}\"\n", pubdir.display(), private.path().display());
    std::fs::write(&roots, text).unwrap();
    let (code, t) = run(&["show", &id]);
    assert!(code == 0 && t.contains("Private note"), "{t}");
    let (code, t) = run(&["new", "guide", "--title", "Another", "--owner", "shop", "--write"]);
    assert_eq!(code, 0, "{t}");
    let names: Vec<_> = std::fs::read_dir(&pubdir).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names, vec![std::ffi::OsString::from(".git")], "nothing written in the public tree");
}

#[test]
fn an_id_prefix_resolves_from_the_user_level_roots_file_and_an_unknown_one_is_named() {
    let other = common::repo();
    let id = other.new_doc("guide", "services/shop", "Over there");
    let app = common::repo();
    let roots = app.path().join("roots.toml");
    let run = |args: &[&str]| {
        let out = app.spec().env("SPECFOLD_ROOTS", &roots).args(args).output().unwrap();
        let both = String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
        (out.status.code().unwrap_or(-1), both)
    };
    std::fs::write(&roots, format!("[prefix]\nlib = \"{}\"\n", other.path().display())).unwrap();
    let (code, t) = run(&["show", &format!("lib:{id}")]);
    assert!(code == 0 && t.contains("Over there"), "{t}");
    let (code, t) = run(&["show", &format!("zz:{id}")]);
    assert!(code == 2 && t.contains("unknown id prefix 'zz'") && t.contains("--root <repo>"), "{t}");
    let (code, t) = run(&["show", &id, "--root", other.path().to_str().unwrap()]);
    assert!(code == 0 && t.contains("Over there"), "{t}");
}
