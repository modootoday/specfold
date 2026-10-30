//! Setup and repair: `spec init`, `spec import`, `spec doctor` and usage errors.

mod common;

#[test]
fn import_brings_an_old_layout_in_and_aliases_resolve() {
    let r = common::repo();
    r.write(
        "docs/adr/0007-use-postgres.md",
        "---\nstatus: accepted\ndate: 2026-01-05\n---\n# Use Postgres\nSee services/shop/src/cart/total.rs\n",
    );
    r.write("docs/old.md", "> Moved to `docs/adr/0007-use-postgres.md`\n");
    r.write(
        "import.toml",
        "[status]\naccepted = \"active\"\n[[sources]]\nglob = \"docs/adr/*.md\"\ntype = \"adr\"\nkey = \"created\"\nname = '^(?P<alias>\\d{4})-(?P<slug>.+)\\.md$'\n[[sources]]\nglob = \"docs/old.md\"\nredirect = 'Moved to `(?P<target>[^`]+)`'\n",
    );
    r.commit("legacy");
    let (code, v) = r.json(&["import", "--map", "import.toml"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["rows"], 2);
    let (code, v) = r.json(&["import", "--map", "import.toml", "--apply", "--write"]);
    assert_eq!(code, 0, "{v}");
    let (code, v) = r.json(&["resolve", "0007"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["id"], "adr:shop/20260105000000");
    assert_eq!(r.json(&["resolve", "docs/old.md"]).1["id"], "adr:shop/20260105000000");
    assert!(!r.path().join("docs/adr/0007-use-postgres.md").exists());
}

#[test]
fn usage_errors_name_the_missing_argument_and_the_nearest_command() {
    let r = common::repo();
    let (code, v) = r.json(&["show"]);
    assert_eq!(code, 2);
    assert!(v["error"].as_str().unwrap().contains("<DOC>"), "{v}");
    let (_, v) = r.json(&["new", "adr"]);
    assert!(v["error"].as_str().unwrap().contains("--title"), "{v}");
    let (_, v) = r.json(&["chek"]);
    assert_eq!(v["next"], "spec help check", "{v}");
}

#[test]
fn doctor_reports_conflict_markers_without_writing() {
    let r = common::repo();
    let (code, v) = r.json(&["doctor"]);
    assert_eq!(code, 0, "{v}");
    let id = r.new_doc("sot", "services/shop", "Cart");
    let (_, v) = r.json(&["show", &id]);
    let path = v["path"].as_str().unwrap().to_string();
    let text = r.read(&path) + "\n<<<<<<< HEAD\na\n=======\nb\n>>>>>>> other\n";
    r.write(&path, &text);
    let (code, v) = r.json(&["doctor"]);
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["items"][0]["check"], "conflict", "{v}");
    assert_eq!(v["items"][0]["path"], path.as_str());
    assert_eq!(r.read(&path), text);
}

#[test]
fn plain_notes_in_spec_local_get_a_way_out() {
    let r = common::repo();
    r.write(".spec.local/AGENTS.md", "# note\n");
    let (_, text) = r.text(&["check"]);
    assert!(text.contains("keep plain notes outside .spec.local"), "{text}");
}

#[test]
fn doctor_finds_local_duplicates() {
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart");
    let (_, v) = r.json(&["show", &id]);
    let path = v["path"].as_str().unwrap().to_string();
    let name = path.rsplit('/').next().unwrap();
    r.write(&format!(".spec.local/shop/sot/{name}"), &r.read(&path));
    let (code, v) = r.json(&["doctor"]);
    assert_eq!(code, 1, "{v}");
    let dup = v["items"].as_array().unwrap().iter().find(|i| i["check"] == "duplicate").expect("duplicate reported");
    assert!(dup["fix"].as_str().unwrap().contains("local copy"), "{v}");
}

#[test]
fn doctor_finds_links_left_by_a_half_done_move() {
    let r = common::repo();
    r.new_doc("sot", "services/shop", "Cart");
    let plan = r.new_doc("plan", "services/shop", "Checkout");
    let (_, v) = r.json(&["show", &plan]);
    let path = v["path"].as_str().unwrap().to_string();
    r.write(
        &path,
        &(r.read(&path)
            + "
See [[sot:blog/cart]].\n"),
    );
    let (code, v) = r.json(&["doctor"]);
    assert_eq!(code, 1, "{v}");
    let m = v["items"].as_array().unwrap().iter().find(|i| i["check"] == "moved").expect("moved reported");
    assert!(m["message"].as_str().unwrap().contains("sot:shop/cart"), "{v}");
    assert!(m["path"].as_str().unwrap().starts_with(&format!("{path}:")), "{v}");
}

#[test]
fn init_upgrade_raises_an_old_schema() {
    let r = common::repo();
    let (_, text) = r.text(&["init", "--upgrade"]);
    assert!(text.contains("is current (schema 1)"), "{text}");
    r.write("spec.toml", &r.read("spec.toml").replacen("schema = 1", "schema = 0", 1));
    let (code, text) = r.text(&["check"]);
    assert_ne!(code, 0);
    assert!(text.contains("spec init --upgrade"), "{text}");
    let (code, v) = r.json(&["init", "--upgrade"]);
    assert_eq!((code, &v["written"]), (0, &serde_json::json!(false)), "{v}");
    let (code, v) = r.json(&["init", "--upgrade", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert!(r.read("spec.toml").contains("schema = 1"));
    assert_eq!(r.json(&["check"]).0, 0);
}

#[test]
fn doctor_says_when_there_is_no_spec_toml() {
    let r = common::repo();
    std::fs::remove_file(r.path().join("spec.toml")).unwrap();
    let (code, v) = r.json(&["doctor"]);
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["next"], "spec init --write", "{v}");
}
