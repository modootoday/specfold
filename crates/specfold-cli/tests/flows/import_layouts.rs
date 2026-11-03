//! `spec import` on the layouts other ADR tools write, from fixtures in tests/fixtures/import.

#[path = "../common/mod.rs"]
mod common;

fn copy_fixture(r: &common::Repo, name: &str) {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/import").join(name);
    let mut stack = vec![base.clone()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                // A directory the project forbids in this repository (`**/decisions`) is stored
                // with a `.fixture` suffix and gets its real name in the test repository.
                let rel = p.strip_prefix(&base).unwrap().to_string_lossy().replace('\\', "/").replace(".fixture/", "/");
                r.write(&rel, &std::fs::read_to_string(&p).unwrap());
            }
        }
    }
}

/// Plan rows in source order (directory listing order varies by file system).
fn rows(v: &serde_json::Value) -> Vec<serde_json::Value> {
    let mut items = v["items"].as_array().unwrap().clone();
    items.sort_by_key(|i| i["source"].as_str().unwrap_or("").to_string());
    items
}

#[test]
fn adr_tools_status_date_and_supersedes_come_from_the_body() {
    let r = common::repo();
    copy_fixture(&r, "adr-tools");
    r.write(
        "import.toml",
        r#"stem_aliases = true
[status]
accepted = "active"
"superceded by" = "superseded"
[body]
status = "section:Status"
date = "line:Date"
[body.links]
Supercedes = "supersedes"
"Superceded by" = "-"
[[sources]]
glob = "doc/adr/*.md"
type = "adr"
key = "created"
name = '^(?P<alias>\d{4})-(?P<slug>.+)\.md$'
"#,
    );
    r.commit("legacy");
    let (code, v) = r.json(&["import", "--map", "import.toml"]);
    assert_eq!(code, 0, "{v}");
    let got: Vec<(String, String, String)> = rows(&v)
        .iter()
        .map(|i| {
            (
                i["status_from"].as_str().unwrap().into(),
                i["status_to"].as_str().unwrap().into(),
                i["id"].as_str().unwrap().into(),
            )
        })
        .collect();
    assert_eq!(
        got,
        vec![
            ("Accepted".into(), "active".into(), "adr:ws/20260112000000".into()),
            ("Superceded by".into(), "superseded".into(), "adr:ws/20260113000000".into()),
            ("Accepted".into(), "active".into(), "adr:ws/20260114000000".into()),
        ]
    );
    let (code, v) = r.json(&["import", "--map", "import.toml", "--apply", "--write"]);
    assert_eq!(code, 0, "{v}");
    let (_, v) = r.json(&["show", "0003", "--rel"]);
    assert!(v.to_string().contains("adr:ws/20260113000000"), "supersedes resolves through the old name: {v}");
    let (_, chain) = r.json(&["graph", "chain", "0002"]);
    assert!(chain.to_string().contains("adr:ws/20260114000000"), "{chain}");
}

#[test]
fn madr2_bullets_set_status_and_date_and_gaps_are_listed() {
    let r = common::repo();
    copy_fixture(&r, "madr2");
    r.write(
        "import.toml",
        r#"[status]
accepted = "active"
"superseded by" = "superseded"
[body]
status = "bullet:Status"
date = "bullet:Date"
[body.links]
"superseded by" = "-"
[[sources]]
glob = "docs/adr/*.md"
type = "adr"
key = "created"
"#,
    );
    r.write("docs/adr/0003-no-bullets.md", "# Plain\n\nNo metadata here.\n");
    r.commit("legacy");
    let (code, v) = r.json(&["import", "--map", "import.toml"]);
    assert_eq!(code, 1, "the file without bullets needs a decision: {v}");
    let items = rows(&v);
    assert_eq!(items[0]["status_to"], "superseded");
    assert_eq!(items[0]["id"], "adr:ws/20260201000000");
    assert_eq!(items[1]["status_to"], "active");
    assert_eq!(items[1]["id"], "adr:ws/20260301000000");
    let gaps = items[2]["unresolved"].to_string();
    assert!(gaps.contains("no status in the body (bullet:Status)") && gaps.contains("bullet:Date"), "{gaps}");
}

#[test]
fn a_bad_body_form_is_refused() {
    let r = common::repo();
    r.write("import.toml", "[body]\nstatus = \"para:Status\"\n");
    let (code, v) = r.json(&["import", "--map", "import.toml"]);
    assert_ne!(code, 0);
    assert!(v.to_string().contains("section:<heading>, bullet:<label> or line:<label>"), "{v}");
}

#[test]
fn madr4_compound_status_matches_a_pattern_and_names_the_replacement() {
    let r = common::repo();
    copy_fixture(&r, "madr4");
    r.write(
        "import.toml",
        r#"[status]
accepted = "active"
[[status_patterns]]
pattern = '^superseded by (?P<by>.+)$'
status = "superseded"
[[sources]]
glob = "docs/decisions/*.md"
type = "adr"
key = "created"
"#,
    );
    r.commit("legacy");
    let (code, v) = r.json(&["import", "--map", "import.toml"]);
    assert_eq!(code, 0, "{v}");
    let items = rows(&v);
    assert_eq!((items[0]["status_to"].as_str(), items[1]["status_to"].as_str()), (Some("superseded"), Some("active")));
    let newer = items[1]["id"].as_str().unwrap();
    assert_eq!(items[0]["notes"][0], format!("replaced by {newer}: rel.supersedes added there"));
    assert!(items[1].get("notes").is_none());
    let (_, text) = r.text(&["import", "--map", "import.toml"]);
    assert!(text.contains(&format!("note: replaced by {newer}")), "{text}");
    // Without the pattern the compound value is a decision, as before.
    r.write(
        "import.toml",
        &r.read("import.toml")
            .replace("[[status_patterns]]", "[[nothing]]")
            .replace("pattern = '^superseded by (?P<by>.+)$'\nstatus = \"superseded\"\n", ""),
    );
    let (code, v) = r.json(&["import", "--map", "import.toml"]);
    assert_ne!(code, 0, "{v}");
}

/// The plan rows for a fixture imported with a preset map as printed.
fn preset_plan(fixture: &str, preset: &str) -> (i32, serde_json::Value, common::Repo) {
    let r = common::repo();
    copy_fixture(&r, fixture);
    let (code, map) = r.text(&["import", "--preset", preset]);
    assert_eq!(code, 0, "{map}");
    assert!(!map.contains("next:"), "the printed map is redirected into a file as is: {map}");
    r.write("import.toml", &map);
    r.commit("legacy");
    let (code, v) = r.json(&["import", "--map", "import.toml"]);
    (code, v, r)
}

fn summary(v: &serde_json::Value) -> Vec<(String, String, String)> {
    rows(v)
        .iter()
        .map(|i| {
            let title = i["source"].as_str().unwrap().rsplit('/').next().unwrap().to_string();
            (title, i["status_to"].as_str().unwrap().to_string(), i["id"].as_str().unwrap().to_string())
        })
        .collect()
}

#[test]
fn presets_import_their_layouts_without_edits() {
    let (code, v, r) = preset_plan("adr-tools", "adr-tools");
    assert_eq!(code, 0, "{v}");
    assert_eq!(summary(&v).iter().map(|s| s.1.as_str()).collect::<Vec<_>>(), ["active", "superseded", "active"]);
    assert_eq!(r.json(&["import", "--map", "import.toml", "--apply", "--write"]).0, 0);
    let (_, d) = r.json(&["show", "0003"]);
    assert_eq!(d["frontmatter"]["title"], "Second Record", "{d}");
    // Written as the id, not the old file name; the old name still resolves as an alias.
    let (_, older) = r.json(&["show", "0002-first-record"]);
    assert_eq!(d["frontmatter"]["rel"]["supersedes"][0], older["id"], "{d}");
    assert!(older["id"].as_str().unwrap().starts_with("adr:"), "{older}");

    let (code, v, _) = preset_plan("madr2", "madr");
    assert_eq!(code, 0, "{v}");
    assert_eq!(summary(&v).iter().map(|s| s.1.as_str()).collect::<Vec<_>>(), ["superseded", "active"]);

    let (code, v, _) = preset_plan("madr4", "madr");
    assert_eq!(code, 0, "{v}");
    assert_eq!(summary(&v).iter().map(|s| s.1.as_str()).collect::<Vec<_>>(), ["superseded", "active"]);

    let (code, v, _) = preset_plan("log4brains", "log4brains");
    assert_eq!(code, 0, "{v}");
    let got = summary(&v);
    let names: Vec<&str> = got.iter().map(|s| s.0.as_str()).collect();
    assert_eq!(names, ["20260601-use-markdown-records.md", "20260701-use-log4brains.md", "20260801-use-a-router.md"]);
    assert_eq!(got.iter().map(|s| s.1.as_str()).collect::<Vec<_>>(), ["superseded", "active", "draft"]);
    assert_eq!(got[0].2, "adr:ws/20260601000000");
}

#[test]
fn a_preset_is_not_combined_with_a_map_and_names_are_checked() {
    let r = common::repo();
    let out = r.spec().args(["import", "--preset", "nope"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let all = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(all.contains("adr-tools"), "{all}");
    let out = r.spec().args(["import", "--preset", "madr", "--map", "x.toml"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let (_, v) = r.json(&["import", "--preset", "madr"]);
    assert!(v["items"][0]["map"].as_str().unwrap().contains("[[status_patterns]]"), "{v}");
}

#[test]
fn adr_tools_preset_reads_the_usual_spelling_links_records_and_rewrites_links() {
    let (code, v, r) = preset_plan("adr-tools-spelling", "adr-tools");
    assert_eq!(code, 0, "{v}");
    let got = summary(&v);
    assert_eq!(got.iter().map(|s| s.1.as_str()).collect::<Vec<_>>(), ["superseded", "active", "active"], "{v}");
    let items = rows(&v);
    let korean = items[2]["notes"].to_string();
    assert!(korean.contains("mostly non-ASCII") && korean.contains("spec mv"), "{korean}");
    assert!(items[1]["notes"].to_string().contains("link(s) to old file names"), "{v}");
    assert!(items[0]["notes"].to_string().contains("drop_status_section"), "{v}");
    assert_eq!(r.json(&["import", "--map", "import.toml", "--apply", "--write"]).0, 0);
    let (_, d) = r.json(&["show", "0002"]);
    // Written as the id, not the old file name; the old name still resolves as an alias.
    let (_, older) = r.json(&["show", "0001-use-files"]);
    assert_eq!(d["frontmatter"]["rel"]["supersedes"][0], older["id"], "{d}");
    assert!(older["id"].as_str().unwrap().starts_with("adr:"), "{older}");
    assert!(d["frontmatter"].get("date").is_none(), "date moves to created only: {d}");
    let (_, first) = r.json(&["show", "0001"]);
    let first_path = first["path"].as_str().unwrap().rsplit('/').next().unwrap().to_string();
    let body = r.read(d["path"].as_str().unwrap());
    assert!(body.contains(&format!("]({first_path}#context)")), "{body}");
    assert!(body.contains("](../guide.md)"), "a link outside the import stays: {body}");
    let (_, k) = r.json(&["show", "0003"]);
    assert_eq!(k["frontmatter"]["title"], "한국어 결정", "{k}");
    let (code, c) = r.json(&["check"]);
    assert_eq!((code, &c["errors"]), (0, &serde_json::json!(0)), "{c}");
    // The kept ../guide.md link no longer leads anywhere from the new folder: L8 says so.
    assert_eq!(rules_of(&c), ["L8"], "{c}");
}

fn rules_of(c: &serde_json::Value) -> Vec<&str> {
    c["items"].as_array().unwrap().iter().map(|f| f["rule"].as_str().unwrap()).collect()
}

#[test]
fn madr_superseded_by_a_link_names_the_newer_record_or_needs_a_decision() {
    let (code, v, r) = preset_plan("madr-linked", "madr");
    assert_eq!(code, 1, "the record outside the import is a decision: {v}");
    assert_eq!(v["unresolved"], 1, "{v}");
    let items = rows(&v);
    let newer = items[1]["id"].as_str().unwrap().to_string();
    assert_eq!(items[0]["notes"][0], format!("replaced by {newer}: rel.supersedes added there"));
    assert!(items[2]["unresolved"][0].as_str().unwrap().contains("does not include"), "{v}");
    let (_, text) = r.text(&["import", "--map", "import.toml"]);
    assert!(text.contains("1 need a decision") && !text.contains("--apply --write"), "{text}");
    // Without the third record the import goes through and check finds no T1.
    std::fs::remove_file(r.path().join("docs/decisions/0003-use-a-log.md")).unwrap();
    let (code, v) = r.json(&["import", "--map", "import.toml"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(r.json(&["import", "--map", "import.toml", "--apply", "--write"]).0, 0);
    let (_, d) = r.json(&["show", "0002"]);
    assert_eq!(d["frontmatter"]["rel"]["supersedes"][0], items[0]["id"], "{d}");
    let (_, c) = r.json(&["check"]);
    assert!(c["by_rule"].get("T1").is_none(), "{c}");
}

#[test]
fn the_plan_names_the_status_line_and_the_alias_setting_before_writing() {
    let r = common::repo();
    copy_fixture(&r, "madr4");
    r.write("import.toml", "[[sources]]\nglob = \"docs/decisions/*.md\"\ntype = \"adr\"\nkey = \"created\"\n[status]\naccepted = \"active\"\n");
    r.commit("legacy");
    let (code, v) = r.json(&["import", "--map", "import.toml"]);
    assert_eq!(code, 1, "{v}");
    let next = v["next"].to_string();
    assert!(next.contains("\\\"superseded by adr-0002\\\" = ") && next.contains("[status]"), "{next}");
    assert!(!next.contains("--apply --write"), "{next}");
    // With every status mapped, a repository without [aliases] file is told what to add.
    r.write("import.toml", &format!("{}\"superseded by adr-0002\" = \"superseded\"\n", r.read("import.toml")));
    let toml = r.read("spec.toml");
    let without: String =
        toml.lines().filter(|l| !l.starts_with("file") && l.trim() != "[aliases]").map(|l| format!("{l}\n")).collect();
    r.write("spec.toml", &without);
    let (code, v) = r.json(&["import", "--map", "import.toml", "--apply"]);
    assert_eq!(code, 0, "{v}");
    assert!(v["next"].to_string().contains("[aliases]"), "{v}");
}

#[test]
fn people_fields_from_madr_and_log4brains_survive_import() {
    for (fixture, preset) in [("madr4", "madr"), ("madr2", "madr"), ("log4brains", "log4brains")] {
        let (_, _, r) = preset_plan(fixture, preset);
        let (code, v) = r.json(&["import", "--map", "import.toml", "--apply", "--write"]);
        assert_eq!(code, 0, "{fixture}: {v}");
        let mut seen = 0;
        for e in std::fs::read_dir(r.path().join(".spec/adr")).unwrap() {
            let t = std::fs::read_to_string(e.unwrap().path()).unwrap();
            assert!(!t.contains("decision-makers:"), "{fixture}: {t}");
            seen += usize::from(t.contains("decision_makers: [team]"));
        }
        assert!(seen >= 1, "{fixture}: no decision_makers kept");
        let (_, ck) = r.json(&["check", "--rule", "S1"]);
        assert!(!ck.to_string().contains("decision"), "{fixture}: {ck}");
    }
}

#[test]
fn c118_madr2_plan_says_which_body_lines_repeat_frontmatter() {
    let (code, v, _) = preset_plan("madr2", "madr");
    assert_eq!(code, 0, "notes do not fail the plan: {v}");
    let notes = v.to_string();
    assert!(notes.contains("the body keeps") && notes.contains("'* Status:'"), "C118: {v}");
}

#[test]
fn c180_import_rewrites_reference_angle_and_encoded_links_but_not_code() {
    let r = common::repo();
    copy_fixture(&r, "madr4");
    let second = "docs/decisions/0002-use-a-database.md";
    let mut text = r.read(second);
    text.push_str(
        "\nSee [ref][old], [angle](<0001-use-plain-files.md#context-and-problem-statement>) and [enc](0001-use%2Dplain-files.md).\n\n[old]: 0001-use-plain-files.md \"the old record\"\n\nKept: `[code](0001-use-plain-files.md)`\n",
    );
    r.write(second, &text);
    let (_, map) = r.text(&["import", "--preset", "madr"]);
    r.write("import.toml", &map);
    r.commit("legacy");
    let (code, v) = r.json(&["import", "--map", "import.toml", "--apply", "--write", "--force"]);
    assert_eq!(code, 0, "{v}");
    let (_, first) = r.json(&["show", "0001"]);
    let new = first["path"].as_str().unwrap().rsplit('/').next().unwrap().to_string();
    let (_, d) = r.json(&["show", "0002"]);
    let body = r.read(d["path"].as_str().unwrap());
    assert!(body.contains(&format!("[old]: {new} \"the old record\"")), "{body}");
    assert!(body.contains(&format!("](<{new}#context-and-problem-statement>)")), "{body}");
    assert!(body.contains(&format!("[enc]({new})")), "{body}");
    assert!(body.contains("`[code](0001-use-plain-files.md)`"), "inline code stays: {body}");
    assert_eq!(body.matches("0001-use").count(), 1, "only the code span keeps the old name: {body}");
}

/// C243: an imported document has one line ending throughout, the source body's (CRLF anywhere
/// makes it CRLF, as C154 keeps an edited file); a BOM is not carried into the new file.
#[test]
fn c243_imported_documents_keep_one_line_ending() {
    let r = common::repo();
    let crlf = "# 1. Pay\r\n\r\nDate: 2026-01-05\r\n\r\n## Status\r\n\r\nAccepted\r\n\r\n## Context\r\n\r\n결제.\r\n";
    r.write("doc/adr/0001-pay.md", crlf);
    r.write("doc/adr/0002-bom.md", &format!("\u{feff}{}", crlf.replace("Pay", "Bom")));
    r.write("doc/adr/0003-lf.md", &crlf.replace("\r\n", "\n").replace("Pay", "Lf"));
    r.write(
        "import.toml",
        "[status]\naccepted = \"active\"\n[body]\nstatus = \"section:Status\"\ndate = \"line:Date\"\n[[sources]]\nglob = \"doc/adr/*.md\"\ntype = \"adr\"\nkey = \"created\"\nname = '^(?P<alias>\\d{4})-(?P<slug>.+)\\.md$'\n",
    );
    r.commit("sources");
    let (code, preview) = r.text(&["import", "--map", "import.toml", "--apply"]);
    assert_eq!(code, 0, "{preview}");
    let (code, v) = r.json(&["import", "--map", "import.toml", "--apply", "--write"]);
    assert_eq!(code, 0, "{v}");
    let made: Vec<String> = walk(&r.dir.path().join(".spec/adr"));
    assert_eq!(made.len(), 3, "{made:?}");
    for path in &made {
        let text = std::fs::read_to_string(path).unwrap();
        let (cr, lf) = (text.matches("\r\n").count(), text.matches('\n').count());
        assert!(!text.starts_with('\u{feff}'), "{path}");
        if path.contains("-lf.") {
            assert_eq!(cr, 0, "{path}: {text:?}");
        } else {
            assert_eq!(cr, lf, "{path} mixes endings: {text:?}");
        }
    }
    {
        let undo = v["undo"].as_str().unwrap_or_default();
        let st = std::process::Command::new("sh").arg("-c").arg(undo).current_dir(r.dir.path()).status().unwrap();
        assert!(st.success(), "{undo}");
        assert_eq!(r.read("doc/adr/0001-pay.md"), crlf, "undo restores the source bytes");
    }
}

fn walk(dir: &std::path::Path) -> Vec<String> {
    let mut out = vec![];
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else if p.extension().is_some_and(|x| x == "md") {
            out.push(p.to_string_lossy().into_owned());
        }
    }
    out
}

#[test]
fn c120_import_dry_run_previews_even_with_apply() {
    let (r, before) = common::madr_repo(true);
    for args in [
        &["import", "--map", "import.toml", "--apply", "--dry-run"][..],
        &["import", "--map", "import.toml", "--dry-run"],
    ] {
        let (code, v) = r.json(args);
        assert_eq!(code, 0, "{args:?}: {v}");
    }
    let o = std::process::Command::new("git").args(["status", "--porcelain"]).current_dir(r.path()).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&o.stdout), before, "nothing written");
    assert_eq!(r.json(&["import", "--map", "import.toml", "--apply", "--dry-run", "--yes"]).0, 2);
    let (_, help) = r.text(&["help", "import"]);
    assert!(help.contains("--dry-run"), "{help}");
}

#[test]
fn c121_import_write_lists_files_and_an_undo_that_works() {
    let (r, before) = common::madr_repo(true);
    let (code, v) = r.json(&["import", "--map", "import.toml", "--apply", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["written"], true, "{v}");
    let changes = v["changes"].to_string();
    assert!(changes.contains(".spec/_aliases.tsv") && changes.contains("adr"), "{v}");
    let undo = v["undo"].as_str().unwrap().to_string();
    assert!(undo.starts_with("rm -rf -- ") && !undo.contains("stash"), "{undo}");
    let ok = std::process::Command::new("sh").args(["-c", &undo]).current_dir(r.path()).status().unwrap();
    assert!(ok.success(), "{undo}");
    let o = std::process::Command::new("git").args(["status", "--porcelain"]).current_dir(r.path()).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&o.stdout), before, "undo restores the tree");
    // No sources: nothing written, no alias table.
    let (r, _) = common::madr_repo(false);
    let (code, v) = r.json(&["import", "--map", "import.toml", "--apply", "--write"]);
    assert_eq!((code, v["written"].clone()), (0, serde_json::json!(false)), "{v}");
    assert!(!r.path().join(".spec/_aliases.tsv").exists());
    assert!(!v.to_string().contains("SPECFOLD_IMPORT"), "{v}");
}

#[test]
fn c123_import_undo_restores_committed_and_uncommitted_sources() {
    let (r, _) = common::madr_repo(true);
    let committed = r.read("docs/adr/0001-use-files.md");
    let loose = "# Untracked\n\n* Status: accepted\n* Date: 2026-03-02\n\n## Context\n\nNot committed.\n";
    r.write("docs/adr/0003-untracked.md", loose);
    let (code, v) = r.json(&["import", "--map", "import.toml", "--apply", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert!(!r.path().join("docs/adr/0003-untracked.md").exists(), "{v}");
    let undo = v["undo"].as_str().unwrap().to_string();
    let out = std::process::Command::new("sh").args(["-c", &undo]).current_dir(r.path()).output().unwrap();
    assert!(out.status.success(), "{undo}: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(r.read("docs/adr/0001-use-files.md"), committed, "{undo}");
    assert_eq!(r.read("docs/adr/0003-untracked.md"), loose, "{undo}");
    assert!(
        !r.path().join(".spec/adr").exists() || std::fs::read_dir(r.path().join(".spec/adr")).unwrap().next().is_none(),
        "{undo}"
    );
    // Text and JSON name the same undo.
    let (r2, _) = common::madr_repo(true);
    r2.write("docs/adr/0003-untracked.md", loose);
    let (_, t) = r2.text(&["import", "--map", "import.toml", "--apply", "--write"]);
    let line = t.lines().find_map(|l| l.trim().strip_prefix("undo: ")).unwrap_or_default();
    assert!(line.contains(".specfold/trash/") && !line.contains("stash"), "{t}");
}

#[test]
fn c138_import_bound_counts_the_alias_table() {
    let (r, _) = common::madr_repo(true);
    let base = r.read("spec.toml");
    // One move (two paths) plus the new alias table is three files.
    r.write("spec.toml", &format!("{base}[write]\nmax_files = 2\n"));
    let (_, v) = r.json(&["import", "--map", "import.toml", "--apply"]);
    assert_eq!(v["files"], 3, "{v}");
    assert_eq!(v["bounded"], true, "{v}");
    r.write("spec.toml", &format!("{base}[write]\nmax_files = 3\n"));
    let (_, v) = r.json(&["import", "--map", "import.toml", "--apply"]);
    assert!(v.get("bounded").is_none(), "{v}");
}

#[test]
fn c155_import_write_refuses_rows_that_need_a_decision() {
    let r = common::repo();
    r.write("docs/adr/0001-use-files.md", "# Use files\n\n* Status: accepted\n\n## Context\n\nWhy.\n");
    let (_, map) = r.text(&["import", "--preset", "madr"]);
    r.write("import.toml", &map);
    r.commit("legacy");
    let (code, v) = r.json(&["import", "--map", "import.toml", "--apply", "--write"]);
    assert_eq!(code, 4, "{v}");
    assert!(r.path().join("docs/adr/0001-use-files.md").exists(), "nothing written: {v}");
    assert!(v["next"].to_string().contains("--force"), "{v}");
    assert!(v.to_string().contains("0001-use-files"), "{v}");
    let (code, v) = r.json(&["import", "--map", "import.toml", "--apply", "--write", "--force"]);
    assert_eq!(code, 0, "{v}");
    assert!(!r.path().join("docs/adr/0001-use-files.md").exists(), "{v}");
}
