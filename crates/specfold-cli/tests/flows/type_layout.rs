//! A type kept in a directory of its own (`[types.<name>] dir`), number keys (`key = "number"`)
//! and a proposal lifecycle written with `[types]` alone.

#[path = "../common/mod.rs"]
mod common;

use serde_json::Value;

const ADR_DIR: &str = "[types.adr]\ndir = \"adr\"\n";
const PROPOSAL: &str =
    "[types.proposal]\nabout = \"An enhancement proposal.\"\nkey = \"number\"\ndir = \"proposals\"\n";

fn add_config(r: &common::Repo, extra: &str) {
    r.write("spec.toml", &(r.read("spec.toml") + extra));
}

fn ids(v: &Value) -> Vec<String> {
    v["items"].as_array().unwrap().iter().map(|i| i["id"].as_str().unwrap().to_string()).collect()
}

fn path_of(r: &common::Repo, id: &str) -> String {
    r.json(&["resolve", id]).1["path"].as_str().unwrap().to_string()
}

/// Exit code and everything printed, for refusals that print no JSON.
fn run(r: &common::Repo, args: &[&str]) -> (i32, String) {
    let out = r.spec().args(args).output().unwrap();
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    (out.status.code().unwrap_or(-1), text)
}

fn cite(r: &common::Repo, from: &str, text: &str) {
    let p = path_of(r, from);
    r.write(&p, &(r.read(&p) + text));
}

#[test]
fn type_dir_documents_are_created_checked_cited_and_counted() {
    let r = common::repo();
    add_config(&r, ADR_DIR);
    let (code, v) = r.json(&["new", "adr", "--title", "Use files", "--write"]);
    assert_eq!(code, 0, "{v}");
    let root_adr = v["id"].as_str().unwrap().to_string();
    let root_path = v["path"].as_str().unwrap().to_string();
    assert!(root_adr.starts_with("adr:ws/"), "{root_adr}");
    assert!(root_path.starts_with("adr/") && root_path.ends_with("-use-files.adr.md"), "{root_path}");
    let shop_adr = r.new_doc("adr", "services/shop/src", "Shop store");
    assert!(shop_adr.starts_with("adr:shop/"), "{shop_adr}");
    assert!(path_of(&r, &shop_adr).starts_with("services/shop/adr/"));
    let sot = r.new_doc("sot", "services/shop/src", "Cart");
    assert!(path_of(&r, &sot).starts_with("services/shop/.spec/sot/"), "other types keep the spec directory");
    cite(&r, &sot, &format!("\nSee [[{root_adr}]] and [[{shop_adr}]], also {root_path}.\n"));
    let (code, v) = r.json(&["check"]);
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["items"][0]["rule"], "L2", "a plain path into the type's directory is a path citation: {v}");
    assert!(v["items"][0]["fix"].to_string().contains(&format!("[[{root_adr}]]")), "{v}");
    let (_, v) = r.json(&["census"]);
    assert_eq!(v["path_cites"], 1, "{v}");
    assert_eq!(r.json(&["fix", "--write"]).0, 0);
    let (code, v) = r.json(&["check"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(ids(&r.json(&["find", "--type", "adr"]).1).len(), 2);
    assert_eq!(r.json(&["resolve", &root_path]).1["id"], root_adr.as_str());
    let (_, v) = r.json(&["census"]);
    assert_eq!(v["by_root"]["adr"], 1, "{v}");
    assert_eq!(v["by_root"]["services/shop/adr"], 1, "{v}");
    assert_eq!(v["strays"], 0, "{v}");
}

#[test]
fn type_dir_strays_and_nested_owner_files_are_told_apart() {
    let r = common::repo();
    add_config(&r, "[types.plan]\ndir = \"services\"\n");
    r.write("services/notes.md", "loose\n");
    let (_, v) = r.json(&["check", "--rule", "S3"]);
    let paths: Vec<&str> = v["items"].as_array().unwrap().iter().map(|f| f["path"].as_str().unwrap()).collect();
    assert_eq!(paths, ["services/notes.md"], "the shop and blog owners' files are theirs: {v}");
}

#[test]
fn type_dir_mv_takes_documents_in_and_out() {
    let r = common::repo();
    let adr = r.new_doc("adr", "services/shop/src", "Pick store");
    let old_path = path_of(&r, &adr);
    assert!(old_path.starts_with("services/shop/.spec/adr/"), "{old_path}");
    let sot = r.new_doc("sot", "services/shop/src", "Cart");
    cite(&r, &sot, &format!("\nSee [[{adr}]] and {old_path}.\n"));
    add_config(&r, ADR_DIR);
    let (code, v) = r.json(&["check", "--rule", "S3"]);
    assert_eq!(code, 1, "{v}");
    let fix = v["items"][0]["fix"].to_string();
    assert!(fix.contains(&format!("spec mv {adr} --write")), "{v}");
    let (code, v) = r.json(&["mv", &adr, "--write"]);
    assert_eq!(code, 0, "{v}");
    let in_dir = path_of(&r, &adr);
    assert!(in_dir.starts_with("services/shop/adr/"), "{in_dir}");
    let sot_text = r.read(&path_of(&r, &sot));
    assert!(!sot_text.contains(&old_path), "the plain path follows the move: {sot_text}");
    assert_eq!(r.json(&["check"]).0, 0);
    let (code, v) = r.json(&["mv", &adr, "--owner", "ws", "--write"]);
    assert_eq!(code, 0, "{v}");
    let root_id = v["to"].as_str().unwrap().to_string();
    assert!(path_of(&r, &root_id).starts_with("adr/"), "{v}");
    assert!(r.read(&path_of(&r, &sot)).contains(&format!("[[{root_id}]]")));
    let (code, v) = r.json(&["mv", &root_id, "--owner", "ws", "--tier", "local", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert!(v["path"].as_str().unwrap().starts_with(".spec.local/ws/adr/"), "the local tier keeps its layout: {v}");
    let (code, v) = r.json(&["mv", &root_id, "--owner", "ws", "--tier", "committed", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert!(v["path"].as_str().unwrap().starts_with("adr/"), "{v}");
}

#[test]
fn type_dir_config_errors_are_refused_with_the_reason() {
    let r = common::repo();
    let base = r.read("spec.toml");
    let cases = [
        ("[types.adr]\ndir = \"decisions\"\n[types.plan]\ndir = \"decisions\"\n", "both have dir = \"decisions\""),
        ("[types.adr]\ndir = \"docs\"\n[types.plan]\ndir = \"docs/plans\"\n", "nest"),
        ("[types.adr]\ndir = \"../adr\"\n", "climbs out of the owner"),
        ("[types.adr]\ndir = \"/srv/adr\"\n", "is absolute"),
        ("[types.adr]\ndir = \".\"\n", "the owner's root itself"),
        ("[types.adr]\ndir = \".spec/decisions\"\n", "overlaps [spec] dir"),
    ];
    for (extra, want) in cases {
        r.write("spec.toml", &(base.clone() + extra));
        let (code, text) = run(&r, &["find"]);
        assert_eq!(code, 2, "{extra}: {text}");
        assert!(text.contains(want) && text.contains("spec explain config"), "{extra}: {text}");
    }
}

#[test]
fn number_keys_take_the_next_free_number_or_the_one_given() {
    let r = common::repo();
    add_config(&r, PROPOSAL);
    let (code, v) = r.json(&["new", "proposal", "--title", "First idea", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["id"], "proposal:ws/0001");
    assert_eq!(v["path"], "proposals/0001-first-idea.proposal.md");
    assert_eq!(r.json(&["new", "proposal", "--title", "Second", "--write"]).1["id"], "proposal:ws/0002");
    let (_, v) = r.json(&["new", "proposal", "--title", "From a pull request", "--number", "42"]);
    assert!(v["next"].to_string().contains("--number 42 --write"), "the preview reruns with the number: {v}");
    let (code, v) = r.json(&["new", "proposal", "--title", "From a pull request", "--number", "42", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["id"], "proposal:ws/0042");
    assert_eq!(r.json(&["new", "proposal", "--title", "After", "--write"]).1["id"], "proposal:ws/0043");
    let (code, text) = run(&r, &["new", "proposal", "--title", "Again", "--number", "42", "--write"]);
    assert_ne!(code, 0, "{text}");
    assert!(text.contains("proposal:ws/0042 already exists") && text.contains("(0044)"), "{text}");
    let (code, text) = run(&r, &["new", "adr", "--title", "Numbered", "--number", "3", "--write"]);
    assert_ne!(code, 0, "{text}");
    assert!(text.contains("--number is for types with key = \"number\""), "{text}");
    assert_eq!(r.json(&["check"]).0, 0);
}

#[test]
fn number_keyed_documents_move_and_keep_their_aliases() {
    let r = common::repo();
    add_config(&r, PROPOSAL);
    let id = r.json(&["new", "proposal", "--title", "Cache", "--write"]).1["id"].as_str().unwrap().to_string();
    let p = path_of(&r, &id);
    r.write(&p, &r.read(&p).replacen(&format!("id: {id}\n"), &format!("id: {id}\naliases: [PROP-1]\n"), 1));
    let (code, v) = r.json(&["mv", &id, "--slug", "scan-cache", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["to"], id.as_str());
    assert_eq!(path_of(&r, &id), "proposals/0001-scan-cache.proposal.md");
    let (code, v) = r.json(&["mv", &id, "--owner", "shop", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["to"], "proposal:shop/0001");
    assert_eq!(r.json(&["resolve", "PROP-1"]).1["id"], "proposal:shop/0001");
    assert_eq!(path_of(&r, "PROP-1"), "services/shop/proposals/0001-scan-cache.proposal.md");
}

#[test]
fn a_proposal_type_runs_draft_accepted_final_or_withdrawn() {
    let r = common::repo();
    add_config(
        &r,
        &format!(
            "{PROPOSAL}statuses = [\"draft\", \"accepted\", \"final\", \"withdrawn\"]\ncurrent = [\"draft\", \"accepted\", \"final\"]\nfields = {{ conformance = \"url\" }}\n[types.proposal.transitions]\ndraft = [\"accepted\", \"withdrawn\"]\naccepted = [\"final\", \"withdrawn\"]\n[types.proposal.require_when]\nfinal = [\"conformance\"]\n"
        ),
    );
    let kept = r.json(&["new", "proposal", "--title", "Kept", "--write"]).1["id"].as_str().unwrap().to_string();
    let dropped = r.json(&["new", "proposal", "--title", "Dropped", "--write"]).1["id"].as_str().unwrap().to_string();
    assert_eq!(r.json(&["set", &kept, "status=accepted", "--write"]).0, 0);
    assert_eq!(r.json(&["set", &kept, "status=final", "--write"]).0, 0);
    let (code, v) = r.json(&["check", "--rule", "S6"]);
    assert_eq!(code, 1, "final needs conformance: {v}");
    assert!(v["items"][0]["message"].to_string().contains("conformance"), "{v}");
    assert_eq!(r.json(&["set", &kept, "conformance=not-a-url", "--write"]).0, 0);
    let (_, v) = r.json(&["check", "--rule", "S8"]);
    assert!(v["items"][0]["message"].to_string().contains("http(s) URL"), "{v}");
    let url = ["set", &kept, "conformance=https://example.com/conformance", "--write"];
    assert_eq!(r.json(&url).0, 0);
    assert_eq!(r.json(&["set", &dropped, "status=withdrawn", "--write"]).0, 0);
    let (code, text) = run(&r, &["set", &dropped, "status=accepted", "--write"]);
    assert_ne!(code, 0, "withdrawn is final: {text}");
    assert_eq!(ids(&r.json(&["find", "--type", "proposal"]).1), std::slice::from_ref(&kept));
    assert_eq!(ids(&r.json(&["find", "--type", "proposal", "--all"]).1).len(), 2);
    assert_eq!(r.json(&["check"]).0, 0);
}

#[test]
fn a_repo_without_type_dirs_or_number_keys_keeps_its_layout() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop/src", "Pick");
    let key = id.rsplit('/').next().unwrap();
    assert!(key.len() == 14 && key.bytes().all(|b| b.is_ascii_digit()), "{id}");
    assert_eq!(path_of(&r, &id), format!("services/shop/.spec/adr/{key}-pick.adr.md"));
    let (_, v) = r.json(&["schema", "adr"]);
    let mut keys: Vec<&str> = v["type"].as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "about",
            "fields",
            "form",
            "initial",
            "key",
            "name",
            "placeholders",
            "require_when",
            "sealed",
            "sections",
            "statuses",
            "template",
            "transitions"
        ],
        "{v}"
    );
    let (_, v) = r.json(&["census"]);
    assert_eq!(v["by_root"], serde_json::json!({"services/shop/.spec": 1}));
    assert_eq!(v["owners_with_docs"], 1);
}

fn finding_paths(v: &Value) -> Vec<String> {
    v["items"].as_array().unwrap().iter().map(|f| f["path"].as_str().unwrap().to_string()).collect()
}

#[test]
fn type_ignore_globs_keep_readmes_and_templates_out_of_s3() {
    let r = common::repo();
    add_config(&r, "[types.adr]\ndir = \"adr\"\nignore = [\"README.md\", \"TEMPLATE.md\"]\n");
    add_config(&r, "[types.sot]\nignore = [\"**/README.md\"]\n");
    r.new_doc("adr", "services/shop/src", "Pick store");
    for f in ["adr/README.md", "adr/TEMPLATE.md", "adr/notes.md", "adr/old/README.md"] {
        r.write(f, "x\n");
    }
    r.write(".spec/sot/README.md", "x\n");
    r.write(".spec/sot/area/README.md", "x\n");
    r.write(".spec/sot/notes.md", "x\n");
    let (code, v) = r.json(&["check", "--rule", "S3"]);
    assert_eq!(code, 1, "{v}");
    let mut paths = finding_paths(&v);
    paths.sort();
    assert_eq!(paths, [".spec/sot/notes.md", "adr/notes.md", "adr/old/README.md"], "{v}");
    let (_, v) = r.json(&["census"]);
    assert_eq!(v["strays"], 3, "{v}");
}

#[test]
fn type_ignore_globs_outside_the_type_dir_are_refused() {
    let r = common::repo();
    let base = r.read("spec.toml");
    for (extra, want) in [
        ("[types.adr]\nignore = [\"../README.md\"]\n", "inside the type's directory"),
        ("[types.adr]\nignore = [\"/README.md\"]\n", "inside the type's directory"),
        ("[types.adr]\nignore = [\"a[\"]\n", "is not a glob"),
    ] {
        r.write("spec.toml", &(base.clone() + extra));
        let (code, text) = run(&r, &["find"]);
        assert_eq!(code, 2, "{extra}: {text}");
        assert!(text.contains("[types.adr] ignore") && text.contains(want), "{extra}: {text}");
    }
}

#[test]
fn url_or_link_fields_take_a_url_or_a_document() {
    let r = common::repo();
    add_config(
        &r,
        &format!(
            "{PROPOSAL}statuses = [\"draft\", \"final\"]\nfields = {{ conformance = \"url|link\" }}\n[types.proposal.transitions]\ndraft = [\"final\"]\n[types.proposal.require_when]\nfinal = [\"conformance\"]\n"
        ),
    );
    let sot = r.new_doc("sot", "services/shop/src", "Conformance report");
    let id = r.json(&["new", "proposal", "--title", "Cache", "--write"]).1["id"].as_str().unwrap().to_string();
    assert_eq!(r.json(&["set", &id, "conformance=no-such-thing", "--write"]).0, 0);
    assert_eq!(r.json(&["set", &id, "status=final", "--write"]).0, 0);
    let (_, v) = r.json(&["check", "--rule", "S8"]);
    let msg = v["items"][0]["message"].to_string();
    assert!(
        msg.contains("conformance should be url|link") && msg.contains("neither an http(s) URL nor a document"),
        "{v}"
    );
    for value in ["https://example.com/report", sot.as_str()] {
        assert_eq!(r.json(&["set", &id, &format!("conformance={value}"), "--write"]).0, 0);
        let (code, v) = r.json(&["check", "--rule", "S8"]);
        assert_eq!((code, v["items"].as_array().unwrap().len()), (0, 0), "{value}: {v}");
    }
    let (_, v) = r.json(&["schema", "proposal"]);
    assert_eq!(v["type"]["kinds"]["conformance"], "url|link", "{v}");
}

#[test]
fn find_and_context_all_name_the_current_statuses() {
    let r = common::repo();
    for cmd in ["find", "context"] {
        let (code, text) = r.text(&[cmd, "--help"]);
        assert_eq!(code, 0);
        assert!(text.contains("current statuses") && text.contains("[types.<name>] current"), "{cmd}: {text}");
    }
}

#[test]
fn v1_finds_type_dirs_that_published_packages_ship() {
    let r = common::repo();
    add_config(&r, ADR_DIR);
    r.write(
        "spec.toml",
        &r.read("spec.toml").replace("[public]\n", "[public]\ndetect = [\"npm-files\", \"cargo-include\"]\n"),
    );
    r.write("services/shop/package.json", "{\"name\": \"shop\", \"files\": [\"dist\", \"adr\"]}\n");
    r.write("libs/auth/Cargo.toml", "[package]\nname = \"auth\"\ninclude = [\"src/**\"]\n");
    r.new_doc("adr", "services/shop/src", "Shop store");
    r.write("libs/auth/src/lib.rs", "\n");
    r.new_doc("adr", "libs/auth/src", "Auth store");
    let (code, v) = r.json(&["check", "--rule", "V1"]);
    assert_eq!(code, 1, "{v}");
    let text = v.to_string();
    assert!(
        text.contains("inside the published path services/shop/adr (npm files in services/shop/package.json)"),
        "{v}"
    );
    assert!(!text.contains("libs/auth/adr"), "the crate includes only src: {v}");
    r.write("services/shop/package.json", "{\"name\": \"shop\", \"files\": [\"dist\"]}\n");
    r.write("libs/auth/Cargo.toml", "[package]\nname = \"auth\"\ninclude = [\"src/**\", \"adr/**\"]\n");
    let (code, v) = r.json(&["check", "--rule", "V1"]);
    assert_eq!(code, 1, "{v}");
    let text = v.to_string();
    assert!(text.contains("published path libs/auth/adr (a published crate, libs/auth/Cargo.toml)"), "{v}");
    assert!(!text.contains("services/shop/adr"), "{v}");
    r.write("libs/auth/Cargo.toml", "[package]\nname = \"auth\"\npublish = false\n");
    r.write("services/shop/package.json", "{\"name\": \"shop\", \"files\": [\"dist\"]}\n");
    assert_eq!(r.json(&["check", "--rule", "V1"]).0, 0);
}

#[test]
fn dropping_a_type_dir_with_documents_left_is_refused_until_they_move() {
    let r = common::repo();
    let base = r.read("spec.toml");
    add_config(&r, ADR_DIR);
    let adr = r.json(&["new", "adr", "--title", "Use files", "--write"]).1["id"].as_str().unwrap().to_string();
    r.write("adr/README.md", "index\n");
    r.commit("adr dir");
    r.write("spec.toml", &base);
    let want = "documents remain in former type dir adr; move them with spec mv first";
    let (code, v) = r.json(&["check"]);
    assert_eq!(code, 1, "{v}");
    assert_eq!(finding_paths(&v).len(), 1, "the README is no stray in a former type dir: {v}");
    assert!(v["items"][0]["message"].to_string().contains(want), "{v}");
    assert!(v["items"][0]["fix"].to_string().contains(&format!("spec mv {adr} --write")), "{v}");
    r.git(&["add", "spec.toml"]);
    let (code, v) = r.json(&["check", "--staged"]);
    assert_eq!(code, 1, "the commit that drops dir is refused: {v}");
    assert!(v.to_string().contains(want), "{v}");
    let (code, v) = r.json(&["mv", &adr, "--write"]);
    assert_eq!(code, 0, "{v}");
    assert!(v["path"].as_str().unwrap().starts_with(".spec/adr/"), "{v}");
    assert_eq!(r.json(&["check"]).0, 0);
    r.commit("adr back in the spec directory");
    assert_eq!(r.json(&["check"]).0, 0);
    assert_eq!(r.json(&["resolve", &adr]).1["id"], adr.as_str());
}
