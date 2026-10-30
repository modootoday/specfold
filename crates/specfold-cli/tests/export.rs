//! `spec export`: the built-in Markdown tree per target (golden trees), its safety checks, and
//! the extended payload an exporter plugin asks for with `wants`.

mod common;

use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

const CHECKOUT: &str = "---\nid: sot:shop/checkout\ntitle: Checkout\nstatus: active\naliases: [checkout-rules]\ncreated: 2026-01-01T00:00:00Z\nupdated: 2026-01-02T00:00:00Z\nrel:\n  governs: [services/shop]\n---\n\n# Checkout\n\nThe decision is [[adr:shop/20260101000000]]; see also [[sot:shop/nowhere]].\n\n## Totals {#CO-1}\n\nTotals include tax. Notes: [[adr:shop/20260102000000]].\n\n```\n[[not:a/link]]\n```\n";
const TOTALS: &str = "---\nid: adr:shop/20260101000000\ntitle: \"Cart totals: tax\"\nstatus: superseded\ncreated: 2026-01-01T00:00:00Z\nupdated: 2026-01-01T00:00:00Z\nrel:\n  governs: [services/shop]\n---\n\n# Cart totals: tax\n\nFollows [[sot:shop/checkout#CO-1|the totals rule]]; `[[sot:shop/checkout]]` is code.\n";
const SCRATCH: &str = "---\nid: adr:shop/20260102000000\ntitle: Scratch\nstatus: draft\ncreated: 2026-01-01T00:00:00Z\nupdated: 2026-01-01T00:00:00Z\n---\n\n# Scratch\n\nBack to [[sot:shop/checkout]].\n";

fn corpus() -> common::Repo {
    let r = common::repo();
    r.write("services/shop/.spec/sot/checkout.sot.md", CHECKOUT);
    r.write("services/shop/.spec/adr/20260101000000-cart-totals.adr.md", TOTALS);
    r.write(".spec.local/shop/adr/20260102000000-scratch.adr.md", SCRATCH);
    r
}

fn tree(dir: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for e in walk(dir) {
        let rel = e.strip_prefix(dir).unwrap().to_string_lossy().replace('\\', "/");
        out.insert(rel, std::fs::read_to_string(&e).unwrap());
    }
    out
}

fn walk(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut v = Vec::new();
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() { v.extend(walk(&p)) } else { v.push(p) }
    }
    v
}

fn golden(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/export").join(name)
}

/// Compares `dir` with the golden tree; `SPECFOLD_BLESS=1` rewrites the golden tree instead.
fn assert_tree(dir: &Path, name: &str) {
    let g = golden(name);
    if std::env::var_os("SPECFOLD_BLESS").is_some() {
        let _ = std::fs::remove_dir_all(&g);
        for (rel, text) in tree(dir) {
            let p = g.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }
    }
    assert_eq!(tree(dir), tree(&g), "export differs from tests/fixtures/export/{name}");
}

#[test]
fn golden_trees_per_target() {
    let r = corpus();
    for t in ["obsidian", "mkdocs", "docusaurus", "github-wiki"] {
        let out = r.path().join("out").join(t);
        let (code, v) = r.json(&["export", "markdown", "--target", t, "--out", out.to_str().unwrap()]);
        assert_eq!(code, 0, "{v}");
        assert_eq!(v["docs"], 2, "the local document is left out without --local");
        let un: Vec<&str> = v["unresolved"].as_array().unwrap().iter().map(|u| u["raw"].as_str().unwrap()).collect();
        assert_eq!(un, ["[[sot:shop/nowhere]]", "[[adr:shop/20260102000000]]"]);
        assert_tree(&out, t);
    }
}

#[test]
fn local_documents_only_with_local_and_reexport_replaces_its_own_files() {
    let r = corpus();
    let out = r.path().join("vault");
    let o = out.to_str().unwrap();
    let (code, v) = r.json(&["export", "markdown", "--target", "obsidian", "--out", o, "--local", "--banner"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["docs"], 3);
    assert!(r.read("vault/shop/sot/checkout.md").contains("[[shop/adr/scratch|Scratch]]"));
    assert!(r.read("vault/shop/adr/scratch.md").starts_with("---\n"));
    assert!(r.read("vault/shop/adr/scratch.md").contains("> Exported by specfold from `.spec.local/shop/adr/"));
    r.write("vault/.obsidian/app.json", "{}");
    let (code, _) = r.json(&["export", "markdown", "--target", "obsidian", "--out", o]);
    assert_eq!(code, 0);
    assert!(!out.join("shop/adr/scratch.md").exists(), "the earlier export's file is removed");
    assert!(out.join(".obsidian/app.json").exists(), "files the export did not write are kept");
}

#[test]
fn refuses_unsafe_output_and_fails_on_unresolved() {
    let r = corpus();
    r.write("notes/keep.md", "mine\n");
    let (code, v) = r.json(&["export", "markdown", "--target", "mkdocs", "--out", "notes"]);
    assert_ne!(code, 0);
    assert!(v["error"].as_str().unwrap_or_default().contains("not empty"), "{v}");
    assert_eq!(r.read("notes/keep.md"), "mine\n");
    let (code, _) = r.json(&["export", "markdown", "--target", "mkdocs", "--out", "."]);
    assert_ne!(code, 0);
    let (code, _) = r.json(&["export", "markdown", "--target", "mkdocs", "--out", "services/shop/.spec/x"]);
    assert_ne!(code, 0);
    let (code, _) = r.json(&["export", "markdown", "--target", "nope", "--out", "x"]);
    assert_ne!(code, 0);
    let (code, v) = r.json(&["export", "markdown", "--target", "mkdocs", "--out", "site", "--fail-on", "unresolved"]);
    assert_eq!(code, 1, "{v}");
    assert!(!r.path().join("site").exists(), "nothing is written");
}

fn plugin(r: &common::Repo, name: &str, wants: &str) -> std::path::PathBuf {
    let seen = r.path().join(format!("{name}.json"));
    let script = r.path().join(format!("{name}.sh"));
    r.write(
        &format!("{name}.sh"),
        &format!(
            "read -r req\ncase \"$req\" in\n*'\"describe\"'*) echo '{{\"protocol\":\"specfold.plugin.v1\",\"name\":\"{name}\",\"wants\":{wants},\"provides\":{{\"exporters\":[\"echo\"]}}}}';;\n*) printf '%s' \"$req\" > '{}'; echo '{{}}';;\nesac\n",
            seen.display()
        ),
    );
    let mut toml = r.read("spec.toml");
    toml.push_str(&format!("[[plugins]]\nname = \"{name}\"\ncmd = [\"sh\", \"{}\"]\n", script.display()));
    r.write("spec.toml", &toml);
    seen
}

fn sent_docs(seen: &Path) -> Vec<Value> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(seen).unwrap()).unwrap();
    let mut docs = v["params"]["docs"].as_array().unwrap().clone();
    docs.sort_by_key(|d| d["id"].as_str().unwrap().to_string());
    docs
}

#[cfg(unix)]
#[test]
fn plugins_get_bodies_and_links_only_when_they_ask() {
    let r = corpus();
    let old = plugin(&r, "plain", "[]");
    let new = plugin(&r, "rich", "[\"body\",\"links\"]");
    assert_eq!(r.json(&["export", "plain"]).0, 0);
    for d in sent_docs(&old) {
        let mut keys: Vec<&String> = d.as_object().unwrap().keys().collect();
        keys.sort();
        assert_eq!(keys, ["anchors", "chapters", "frontmatter", "id", "owner", "path", "status", "title", "type"]);
    }
    let (code, v) = r.json(&["export", "rich"]);
    assert_eq!(code, 0, "{v}");
    let docs = sent_docs(&new);
    let checkout = docs.iter().find(|d| d["id"] == "sot:shop/checkout").unwrap();
    let body = checkout["body"].as_str().unwrap_or_else(|| panic!("{checkout}"));
    assert!(body.trim_start().starts_with("# Checkout"), "{body:?}");
    assert!(checkout["hash"].as_str().unwrap().starts_with("sha256:"));
    let links = checkout["links"].as_array().unwrap();
    let first = &links[0];
    let (a, b) = (first["span"][0].as_u64().unwrap() as usize, first["span"][1].as_u64().unwrap() as usize);
    assert_eq!(&body[a..b], "[[adr:shop/20260101000000]]");
    assert_eq!(first["target_id"], "adr:shop/20260101000000");
    assert_eq!(first["status"], "superseded");
    assert_eq!(first["title"], "Cart totals: tax");
    assert_eq!(links[1]["reason"], "names no document");
    assert_eq!(links.len(), 3, "the fenced example is not a reference");
    let totals = docs.iter().find(|d| d["id"] == "adr:shop/20260101000000").unwrap();
    assert_eq!(totals["links"][0]["anchor"], "CO-1");
    assert_eq!(totals["links"][0]["label"], "the totals rule");
    // Over the byte budget, bodies are handed over as paths.
    let (code, _) = r.json(&["export", "rich", "--max-bytes", "10"]);
    assert_eq!(code, 0);
    for d in sent_docs(&new) {
        assert!(d.get("body").is_none());
        assert_eq!(d["body_path"], d["path"]);
        assert!(d["links"].is_array());
    }
}

#[cfg(unix)]
#[test]
fn the_markdown_name_is_reserved() {
    let r = corpus();
    plugin(&r, "markdown", "[]");
    let (code, v) = r.json(&["plugins"]);
    assert!(v.to_string().contains("reserved for the built-in exporter"), "{code} {v}");
}

#[cfg(unix)]
#[test]
fn plugins_get_local_documents_only_with_local() {
    let r = corpus();
    let seen = plugin(&r, "rich", "[\"body\",\"links\"]");
    let scratch = "adr:shop/20260102000000";
    assert_eq!(r.json(&["export", "rich"]).0, 0);
    let raw = std::fs::read_to_string(&seen).unwrap();
    let docs = sent_docs(&seen);
    assert_eq!(docs.len(), 2, "the local document stays out");
    assert!(!raw.contains("Scratch") && !raw.contains(".spec.local"), "nothing about it leaks: {raw}");
    let checkout = docs.iter().find(|d| d["id"] == "sot:shop/checkout").unwrap();
    let to_scratch = &checkout["links"][2];
    assert_eq!(to_scratch["reason"], "not exported");
    assert!(to_scratch["target_id"].is_null() && to_scratch["resolved_path"].is_null());
    let v: Value = serde_json::from_str(&raw).unwrap();
    assert!(v["params"]["edges"].as_array().unwrap().iter().all(|e| e["from"] != scratch && e["to"] != scratch));
    assert_eq!(r.json(&["export", "rich", "--local"]).0, 0);
    let docs = sent_docs(&seen);
    assert_eq!(docs.len(), 3);
    assert!(docs.iter().any(|d| d["id"] == scratch));
    let plain = plugin(&r, "plain", "[]");
    assert_eq!(r.json(&["export", "plain"]).0, 0);
    assert_eq!(sent_docs(&plain).len(), 2);
}

/// A password assembled at run time so this file holds nothing a secret scanner flags.
fn leaky_url() -> String {
    format!("postgres://app:{}{}@db.acme.io/main", "s3cr", "Et-77x")
}

fn with_public_guard(r: &common::Repo) {
    let mut toml = r.read("spec.toml");
    toml.push_str(
        "[[guards]]\nname = \"public\"\npublic = true\nsecrets = true\nrange = \"all\"\nfail_if_empty = false\n",
    );
    r.write("spec.toml", &toml);
}

#[test]
fn the_public_guard_stops_an_export_that_holds_a_secret() {
    let r = corpus();
    with_public_guard(&r);
    let (code, v) = r.json(&["export", "markdown", "--target", "mkdocs", "--out", "site"]);
    assert_eq!(code, 0, "a clean corpus exports: {v}");
    let path = "services/shop/.spec/sot/checkout.sot.md";
    r.write(path, &(CHECKOUT.to_string() + "\nDSN: " + &leaky_url() + "\n"));
    let (code, v) = r.json(&["export", "markdown", "--target", "mkdocs", "--out", "site2"]);
    assert_eq!(code, 1, "{v}");
    assert!(!r.path().join("site2").exists(), "nothing is written");
    let s = v.to_string();
    assert!(s.contains("connection string") && !s.contains("Et-77x"), "{s}");
    let (code, v) = r.json(&["export", "markdown", "--target", "mkdocs", "--out", "site2", "--no-guard"]);
    assert_eq!(code, 0, "{v}");
}

#[cfg(unix)]
#[test]
fn the_public_guard_runs_before_a_plugin_is_called() {
    let r = corpus();
    with_public_guard(&r);
    let seen = plugin(&r, "rich", "[\"body\"]");
    let path = "services/shop/.spec/sot/checkout.sot.md";
    r.write(path, &(CHECKOUT.to_string() + "\nDSN: " + &leaky_url() + "\n"));
    let (code, v) = r.json(&["export", "rich"]);
    assert_eq!(code, 1, "{v}");
    assert!(!seen.exists(), "the plugin never saw the payload");
    r.write(path, &(CHECKOUT.to_string() + "\nDSN: postgres://app:${DB_PASSWORD}@db.acme.io/main\n"));
    let (code, v) = r.json(&["export", "rich"]);
    assert_eq!(code, 0, "a placeholder passes: {v}");
    assert!(seen.exists());
}
