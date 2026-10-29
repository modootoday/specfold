//! `spec import` on the layouts other ADR tools write, from fixtures in tests/fixtures/import.

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
                let rel = p.strip_prefix(&base).unwrap().to_string_lossy().replace('\\', "/");
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
