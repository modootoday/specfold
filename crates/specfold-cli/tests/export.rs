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
    let (code, v) = r.json(&["export", "markdown", "--target", "obsidian", "--out", o]);
    assert_eq!(code, 4, "CLI-17: a refused write");
    assert_eq!(v["written"], false, "replacing an earlier export needs --yes (CLI-3): {v}");
    assert!(v["next"].as_str().unwrap().ends_with("--yes"), "{v}");
    assert!(out.join("shop/adr/scratch.md").exists(), "nothing is removed without --yes");
    let (code, _) = r.json(&["export", "markdown", "--target", "obsidian", "--out", o, "--yes"]);
    assert_eq!(code, 0);
    assert!(!out.join("shop/adr/scratch.md").exists(), "the earlier export's file is removed");
    assert!(out.join(".obsidian/app.json").exists(), "files the export did not write are kept");
}

#[test]
fn c114_c115_dry_run_writes_nothing_and_previews_a_replacing_export() {
    let r = corpus();
    let out = r.path().join("vault");
    let o = out.to_str().unwrap();
    let (code, v) = r.json(&["export", "markdown", "--target", "obsidian", "--out", o, "--dry-run"]);
    assert_eq!((code, v["written"].clone()), (0, serde_json::json!(false)), "C114: {v}");
    assert!(!out.exists(), "C114: a dry run writes nothing");
    assert!(v["files"].as_u64().unwrap() > 0, "{v}");
    assert_eq!(r.json(&["export", "markdown", "--target", "obsidian", "--out", o]).0, 0);
    let before = tree(&out);
    let (code, v) = r.json(&["export", "markdown", "--target", "obsidian", "--out", o, "--dry-run"]);
    assert_eq!(code, 0, "C115: previewing a replacing export is not refused: {v}");
    assert!(!v["replaces"].as_array().unwrap().is_empty(), "{v}");
    assert!(v["next"].as_str().unwrap().ends_with("--yes"), "{v}");
    assert_eq!(tree(&out), before);
    let (code, _) = r.json(&["export", "markdown", "--target", "obsidian", "--out", o, "--dry-run", "--yes"]);
    assert_eq!(code, 2, "--dry-run --yes stays a usage error");
}

#[test]
fn refuses_unsafe_output_and_fails_on_unresolved() {
    let r = corpus();
    r.write("notes/keep.md", "mine\n");
    let (code, v) = r.json(&["export", "markdown", "--target", "mkdocs", "--out", "notes"]);
    assert_ne!(code, 0);
    assert!(v["error"].as_str().unwrap_or_default().contains("not empty"), "{v}");
    assert_eq!(v["next"], "spec export markdown --target mkdocs --out notes-export", "{v}");
    assert_eq!(r.read("notes/keep.md"), "mine\n");
    let (_, v) = r.json(&["export", "markdown", "--out", "site"]);
    assert_eq!(v["next"], "spec export markdown --target obsidian --out site", "{v}");
    let (code, v) = r.json(&["export", "markdown", "--target", "mkdocs", "--out", "fresh"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["next"], "spec check --rule L1", "two references lead nowhere: {v}");
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
    format!("postgres:{}app:{}{}@db.acme.io/main", "//", "s3cr", "Et-77x")
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

#[test]
fn the_built_in_secrets_check_runs_with_no_guard_configured() {
    let r = corpus();
    let path = "services/shop/.spec/sot/checkout.sot.md";
    r.write(path, &(CHECKOUT.to_string() + "\nDSN: " + &leaky_url() + "\n"));
    let (code, v) = r.json(&["export", "markdown", "--target", "mkdocs", "--out", "site"]);
    assert_eq!(code, 1, "{v}");
    assert!(!r.path().join("site").exists(), "nothing is written");
    let s = v.to_string();
    assert!(s.contains("built-in check") && !s.contains("Et-77x"), "{s}");
    assert_eq!(v["next"], "spec guard secrets", "{v}");
    // The hit names the source file and its line, not the exported copy.
    let line = r.read(path).lines().position(|l| l.starts_with("DSN:")).unwrap() + 1;
    assert_eq!(v["guard"][0]["path"], path, "{v}");
    assert_eq!(v["guard"][0]["line"], line, "{v}");
    let (code, g) = r.json(&["guard", "secrets"]);
    assert_eq!(code, 1, "spec guard secrets agrees with export: {g}");
    assert!(g.to_string().contains(path), "{g}");
    let (_, t) = r.text(&["export", "markdown", "--target", "mkdocs", "--out", "site"]);
    assert!(t.contains("pass --no-guard"), "{t}");
    let (code, v) = r.json(&["export", "markdown", "--target", "mkdocs", "--out", "site", "--no-guard"]);
    assert_eq!(code, 0, "{v}");
}

#[test]
fn a_guard_and_the_built_in_check_report_a_line_once() {
    let r = corpus();
    with_public_guard(&r);
    let path = "services/shop/.spec/sot/checkout.sot.md";
    r.write(path, &(CHECKOUT.to_string() + "\nDSN: " + &leaky_url() + "\n"));
    let (code, v) = r.json(&["export", "markdown", "--target", "mkdocs", "--out", "site"]);
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["guard"].as_array().map(|a| a.len()), Some(1), "{v}");
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

#[test]
fn a_markdown_link_to_an_id_is_warned_about_and_rewritten_on_export() {
    let r = common::repo();
    let a = CHECKOUT.replace(
        "The decision is [[adr:shop/20260101000000]]; see also [[sot:shop/nowhere]].",
        "The decision is [the tax rule](adr:shop/20260101000000); also [gone](adr:shop/1999).",
    );
    let a = a.replace("Notes: [[adr:shop/20260102000000]].", "Notes.");
    r.write("services/shop/.spec/sot/checkout.sot.md", &a);
    r.write("services/shop/.spec/adr/20260101000000-cart-totals.adr.md", TOTALS);
    let (_, v) = r.json(&["check"]);
    let s = v.to_string();
    assert!(s.contains("[[adr:shop/20260101000000|the tax rule]]"), "{s}");
    assert!(s.contains("[gone](adr:shop/1999) is a Markdown link to a document id that names no document"), "{s}");
    let (code, v) = r.json(&["export", "markdown", "--target", "mkdocs", "--out", "site"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["next"], "git status --short -- site", "{v}");
    let page = r.read("site/shop/sot/checkout.md");
    assert!(page.contains("[the tax rule](../adr/cart-totals.md)"), "{page}");
    assert!(page.contains("[gone](adr:shop/1999)"), "left as written: {page}");
}

#[cfg(unix)]
#[test]
fn the_built_in_check_stops_a_plugin_with_no_guard_until_no_guard_is_passed() {
    let r = corpus();
    let seen = plugin(&r, "rich", "[\"body\"]");
    let path = "services/shop/.spec/sot/checkout.sot.md";
    r.write(path, &(CHECKOUT.to_string() + "\nDSN: " + &leaky_url() + "\n"));
    let (code, v) = r.json(&["export", "rich"]);
    assert_eq!(code, 1, "{v}");
    assert!(!seen.exists(), "the plugin never saw the payload");
    let (code, v) = r.json(&["export", "rich", "--no-guard"]);
    assert_eq!(code, 0, "{v}");
    assert!(std::fs::read_to_string(&seen).unwrap().contains("Et-77x"), "--no-guard sends the body as written");
}

#[test]
fn a_warn_level_public_guard_lists_its_hit_and_exports() {
    let r = corpus();
    with_public_guard(&r);
    r.write("spec.toml", &(r.read("spec.toml") + "level = \"warn\"\n"));
    let path = "services/shop/.spec/sot/checkout.sot.md";
    r.write(path, &(CHECKOUT.to_string() + "\nDSN: " + &leaky_url() + "\n"));
    let (code, v) = r.json(&["export", "markdown", "--target", "mkdocs", "--out", "site"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["guard"][0]["level"], "warn", "{v}");
    assert!(!v.to_string().contains("Et-77x"), "{v}");
    assert!(r.path().join("site/shop/sot/checkout.md").exists());
}

#[test]
fn an_unknown_target_and_a_missing_out_name_a_real_next_command() {
    let r = corpus();
    let (code, v) = r.json(&["export", "markdown", "--target", "nope", "--out", "x"]);
    assert_ne!(code, 0);
    let err = v["error"].as_str().unwrap_or_default();
    assert!(err.contains("nope") && err.contains("obsidian") && err.contains("mkdocs"), "{v}");
    assert!(v["next"].as_str().unwrap_or_default().starts_with("spec "), "{v}");
    let (code, v) = r.json(&["export", "markdown", "--target", "mkdocs"]);
    assert_ne!(code, 0);
    assert!(v.to_string().contains("--out"), "{v}");
    assert!(v["next"].as_str().unwrap_or_default().starts_with("spec "), "{v}");
}

fn with_md_links() -> common::Repo {
    let r = common::repo();
    let a = CHECKOUT
        .replace(
            "The decision is [[adr:shop/20260101000000]]; see also [[sot:shop/nowhere]].",
            "The decision is [the tax rule](adr:shop/20260101000000); `[code](adr:shop/20260101000000)` stays.",
        )
        .replace("Notes: [[adr:shop/20260102000000]].", "Notes.");
    r.write("services/shop/.spec/sot/checkout.sot.md", &a);
    r.write("services/shop/.spec/adr/20260101000000-cart-totals.adr.md", TOTALS);
    r
}

#[test]
fn fix_rewrites_a_markdown_link_to_an_id_as_a_wiki_link() {
    let r = with_md_links();
    let (code, v) = r.json(&["fix", "--rule", "L1", "--write"]);
    assert_eq!(code, 0, "{v}");
    let text = r.read("services/shop/.spec/sot/checkout.sot.md");
    assert!(text.contains("[[adr:shop/20260101000000|the tax rule]]"), "{text}");
    assert!(text.contains("`[code](adr:shop/20260101000000)`"), "code is left alone: {text}");
    let (_, v) = r.json(&["check"]);
    assert!(!v.to_string().contains("is a Markdown link to a document"), "{v}");
}

#[test]
fn a_markdown_link_to_an_id_inside_code_is_not_warned_about() {
    let r = with_md_links();
    let (_, v) = r.json(&["check"]);
    let s = v.to_string();
    assert_eq!(s.matches("is a Markdown link to a document").count(), 1, "{s}");
}

#[test]
fn obsidian_turns_a_markdown_link_to_an_id_into_a_wiki_link() {
    let r = with_md_links();
    let (code, v) = r.json(&["export", "markdown", "--target", "obsidian", "--out", "vault"]);
    assert_eq!(code, 0, "{v}");
    let page = r.read("vault/shop/sot/checkout.md");
    assert!(page.contains("[[shop/adr/cart-totals|the tax rule]]"), "{page}");
}

#[test]
fn a_markdown_link_to_a_slug_id_points_at_the_exported_page() {
    let r = corpus();
    r.write(
        "services/shop/.spec/sot/refunds.sot.md",
        "---\nid: sot:shop/refunds\ntitle: Refunds\nstatus: active\ncreated: 2026-01-01T00:00:00Z\nupdated: 2026-01-01T00:00:00Z\n---\n\n# Refunds\n\nTotals follow [the tax decision](adr:shop/cart-totals).\n",
    );
    let (_, v) = r.json(&["check", "--rule", "L1"]);
    let s = v.to_string();
    assert!(s.contains("[[adr:shop/20260101000000|the tax decision]]"), "L1 resolves the slug id: {s}");
    assert!(!s.contains("cart-totals) is a Markdown link to a document id that names no document"), "{s}");
    let (code, v) = r.json(&["export", "markdown", "--target", "mkdocs", "--out", "site"]);
    assert_eq!(code, 0, "{v}");
    let page = r.read("site/shop/sot/refunds.md");
    assert!(page.contains("[the tax decision](../adr/cart-totals.md)"), "{page}");
    assert!(!page.contains("adr:shop/cart-totals"), "{page}");
}

#[test]
fn the_built_in_check_reads_repository_attachments_of_exported_documents() {
    let r = corpus();
    let (code, v) = r.json(&["new", "design", "--governs", "services/shop", "--title", "Board"]);
    assert_eq!(code, 0, "{v}");
    let id = v["id"].as_str().unwrap().to_string();
    r.write("notes.txt", &format!("DSN: {}\n", leaky_url()));
    let mut bin = b"\0\x01PNG ".to_vec();
    bin.extend(leaky_url().bytes());
    std::fs::write(r.path().join("board.bin"), &bin).unwrap();
    let (code, v) = r.json(&["attach", &id, "notes.txt", "board.bin", "--write"]);
    assert_eq!(code, 0, "{v}");
    let (code, v) = r.json(&["export", "markdown", "--target", "mkdocs", "--out", "site"]);
    assert_eq!(code, 1, "the attachment holds a secret: {v}");
    assert!(!r.path().join("site").exists(), "nothing is written");
    let s = v.to_string();
    assert!(s.contains("notes.txt") && !s.contains("board.bin") && !s.contains("Et-77x"), "{s}");
    r.write("spec.toml", &(r.read("spec.toml") + "\n[files]\nscan_max_bytes = 8\n"));
    let (code, v) = r.json(&["export", "markdown", "--target", "mkdocs", "--out", "site"]);
    assert_eq!(code, 0, "over scan_max_bytes the file is not read: {v}");
    let note = v["not_scanned"].as_str().unwrap_or_default();
    assert!(note.starts_with("2 attachment(s) not scanned") && note.contains("board.bin"), "{v}");
}

#[test]
fn c104_an_out_outside_the_repository_is_listed_not_git_statused() {
    let r = common::repo();
    r.new_doc("sot", "services/shop/src/cart", "Cart totals");
    r.commit("docs");
    let outside = tempfile::tempdir().unwrap();
    let out = outside.path().join("vault");
    let (code, v) = r.json(&["export", "markdown", "--target", "obsidian", "--out", out.to_str().unwrap()]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["unresolved"].as_array().unwrap().len(), 0, "{v}");
    assert_eq!(v["next"], format!("ls {}", out.display()), "git cannot see outside the repository: {v}");
    let (code, v) = r.json(&["export", "markdown", "--target", "obsidian", "--out", "vault"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["next"], "git status --short -- vault", "inside, git status still applies: {v}");
}

#[test]
fn c166_refused_export_preview_next_stays_a_preview() {
    let r = corpus();
    let (code, v) = r.json(&["export", "markdown", "--out", "f", "--dry-run"]);
    assert_eq!(code, 2, "{v}");
    let next = v["next"].as_str().unwrap().to_string();
    assert!(next.contains("--target") && next.contains("--dry-run"), "{v}");
    let args: Vec<&str> = next.split(' ').skip(1).collect();
    assert_eq!(r.json(&args).0, 0);
    assert!(!r.path().join("f").exists(), "following the next wrote f/");
}

#[test]
fn c177_export_preview_next_keeps_the_content_flags_but_not_no_guard() {
    let r = corpus();
    let out = r.path().join("vault");
    let o = out.to_str().unwrap();
    // --fail-on rides the same helper; this corpus has an unresolved ref, so it stays off here.
    let flags = ["--banner", "--local"];
    let mut args = vec!["export", "markdown", "--target", "obsidian", "--out", o];
    args.extend(flags);
    args.extend(["--no-guard", "--dry-run"]);
    let (code, preview) = r.json(&args);
    assert_eq!(code, 0, "{preview}");
    let next = preview["next"].as_str().unwrap().to_string();
    assert!(next.contains(" --banner") && next.contains(" --local"), "{next}");
    assert!(!next.contains("--dry-run") && !next.contains("--no-guard"), "{next}");
    let words: Vec<&str> = next.split(' ').skip(1).collect();
    let (code, wrote) = r.json(&words);
    assert_eq!(code, 0, "{wrote}");
    assert_eq!(wrote["docs"], preview["docs"], "the write matches the preview");
}

#[test]
fn c189_local_and_no_local_together_are_refused_in_either_order() {
    let r = corpus();
    for args in [["--local", "--no-local"], ["--no-local", "--local"]] {
        let mut a = vec!["export", "markdown", "--target", "obsidian", "--out", "o", "--dry-run"];
        a.extend(args);
        let (code, text) = r.text(&a);
        assert_eq!(code, 2, "{text}");
        assert!(!r.path().join("o").exists());
    }
}
