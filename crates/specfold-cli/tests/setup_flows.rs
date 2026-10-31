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

#[test]
fn c169_init_upgrade_reads_a_commented_table_header() {
    let r = common::repo();
    let text = r.read("spec.toml").replacen("[spec]", "[spec] # main", 1).replacen("schema = 1", "schema = 0", 1);
    r.write("spec.toml", &text);
    assert_eq!(r.json(&["check"]).0, 2);
    let (code, v) = r.json(&["init", "--upgrade", "--write"]);
    assert_eq!((code, &v["written"]), (0, &serde_json::json!(true)), "{v}");
    assert!(r.read("spec.toml").contains("[spec] # main\nschema = 1"));
    assert_eq!(r.json(&["check"]).0, 0);
}

#[test]
fn c170_init_upgrade_refuses_a_newer_schema_like_check() {
    let r = common::repo();
    r.write("spec.toml", &r.read("spec.toml").replacen("schema = 1", "schema = 7", 1));
    let (code, v) = r.json(&["check"]);
    let (code2, v2) = r.json(&["init", "--upgrade"]);
    assert_eq!((code, code2), (2, 2), "{v} {v2}");
    assert_eq!(v["error"], v2["error"], "{v2}");
    assert!(v2["error"].as_str().unwrap().contains("install the latest spec"), "{v2}");
    assert!(r.read("spec.toml").contains("schema = 7"));
}

#[test]
fn c193_dir_naming_a_file_is_refused_and_an_empty_dir_points_at_the_typed_command() {
    let r = common::repo();
    r.write("afile", "x\n");
    let (code, text) = r.text(&["-C", "afile", "find"]);
    assert_eq!(code, 3, "{text}");
    assert!(text.contains("is a file") && text.contains("next: spec help find"), "{text}");
    let (code, text) = r.text(&["-C", "", "find"]);
    assert_eq!(code, 2, "{text}");
    assert!(text.contains("next: spec help find"), "{text}");
}

#[test]
fn c191_upgrade_reads_past_a_bom_and_keeps_it() {
    let r = common::repo();
    let cur = r.read("spec.toml");
    let old = cur.replacen("schema = 1", "schema = 0", 1);
    assert_ne!(cur, old, "fixture has schema = 1");
    r.write("spec.toml", &format!("\u{feff}{old}"));
    assert_ne!(r.text(&["check"]).0, 0);
    let (code, text) = r.text(&["init", "--upgrade", "--write"]);
    assert_eq!(code, 0, "{text}");
    assert!(!text.contains("is current"), "{text}");
    let now = r.read("spec.toml");
    assert!(now.starts_with('\u{feff}') && now.contains("schema = 1"), "{now:?}");
    let (code, text) = r.text(&["check"]);
    assert_eq!(code, 0, "{text}");
}

#[test]
fn c210_owner_patterns_take_whole_star_segments() {
    let fresh = tempfile::tempdir().unwrap();
    for bad in ["services/**={1}", "services/svc-*=svc.{1}"] {
        let r = common::repo();
        let out = r
            .spec()
            .current_dir(fresh.path())
            .args(["init", "--owner-kind", bad, "--write", "--json"])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2), "{bad}: {}", String::from_utf8_lossy(&out.stdout));
        assert!(String::from_utf8_lossy(&out.stdout).contains("whole '*' segments"), "{bad}");
    }
    let r = common::repo();
    let toml = r.read("spec.toml") + "\n[[owners.kinds]]\npattern = \"libs/lib-*\"\nid = \"{1}\"\n";
    r.write("spec.toml", &toml);
    let (code, v) = r.json(&["check"]);
    assert_eq!(code, 2, "{v}");
    assert!(v["error"].as_str().unwrap().contains("whole '*' segments"), "{v}");
}

/// C235: a spec.toml that does not load: doctor points at its line, and `init --upgrade` fails.
#[test]
fn c235_broken_config_points_at_the_line_and_upgrade_refuses() {
    let r = common::repo();
    r.write("spec.toml", &(r.read("spec.toml") + "\n[write]\nmax_files = \"x\"\n"));
    let line = r.read("spec.toml").lines().position(|l| l.starts_with("max_files")).unwrap() + 1;
    let (_, v) = r.json(&["doctor"]);
    let fix = v["items"][0]["fix"].as_str().unwrap();
    assert_eq!(fix, format!("fix spec.toml at line {line} (spec explain config)"), "{v}");
    let (code, text) = r.text(&["init", "--upgrade"]);
    assert_eq!(code, 2, "{text}");
    assert!(!text.contains("is current") && text.contains("next: spec doctor"), "{text}");
}
