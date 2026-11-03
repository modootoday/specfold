//! Changing documents: `spec new`, `set`, `edit`, `mv` and `fix`.

#[path = "../common/mod.rs"]
mod common;

use serde_json::{Value, json};

#[test]
fn supersede_flow_keeps_both_sides_in_step() {
    let r = common::repo();
    let old = r.new_doc("adr", "services/shop", "Use files");
    assert_eq!(r.json(&["set", &old, "status=active", "--write"]).0, 0);
    let (code, v) =
        r.json(&["new", "adr", "--from", &old, "--governs", "services/shop", "--title", "Use a database", "--write"]);
    assert_eq!(code, 0, "{v}");
    let new = v["id"].as_str().unwrap().to_string();
    assert_eq!(r.json(&["show", &old]).1["frontmatter"]["status"], "active", "the draft has not taken effect");
    assert_eq!(r.json(&["set", &new, "status=active", "--write"]).0, 0);
    assert_eq!(r.json(&["show", &old]).1["frontmatter"]["status"], "superseded");
    let (_, chain) = r.json(&["graph", "chain", &old]);
    assert_eq!(chain["current"], new);
    assert_eq!(r.json(&["check"]).0, 0, "{}", r.json(&["check"]).1);
}

#[test]
fn mv_rewrites_references() {
    let r = common::repo();
    let sot = r.new_doc("sot", "services/shop", "Cart");
    let plan = r.new_doc("plan", "services/shop", "Ship");
    let plan_path = r.json(&["resolve", &plan]).1["path"].as_str().unwrap().to_string();
    let text = r.read(&plan_path) + &format!("\nSee [[{sot}]] and [[{sot}#X|x]].\n");
    r.write(&plan_path, &text);
    assert_eq!(r.json(&["mv", &sot, "--slug", "basket", "--write"]).0, 0);
    let after = r.read(&plan_path);
    assert!(after.contains("[[sot:shop/basket]]") && after.contains("[[sot:shop/basket#X|x]]"), "{after}");
}

#[test]
fn edit_previews_then_replaces_and_appends_sections() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Pay by card");
    let (_, v) = r.json(&["show", &id]);
    let path = v["path"].as_str().unwrap().to_string();
    let before = r.read(&path);
    let (code, v) = r.json(&["edit", &id, "--section", "Decision", "--text", "Use cards."]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(r.read(&path), before, "preview must not write");
    let (code, v) = r.json(&["edit", &id, "--section", "Decision", "--text", "Use cards.", "--write"]);
    assert_eq!(code, 0, "{v}");
    let (code, _) = r.json(&["edit", &id, "--section", "decision", "--append", "--text", "And refunds.", "--write"]);
    assert_eq!(code, 0);
    let (code, _) = r.json(&["edit", &id, "--append", "--text", "## Addendum\n\nLater note.", "--write"]);
    assert_eq!(code, 0);
    let text = r.read(&path);
    assert!(text.contains("## Decision\n\nUse cards.\n\nAnd refunds.\n\n## Consequences"), "{text}");
    assert!(text.trim_end().ends_with("## Addendum\n\nLater note."), "{text}");
    let (code, v) = r.json(&["edit", &id, "--section", "Nope", "--text", "x"]);
    assert_ne!(code, 0, "{v}");
    let (code, v) = r.json(&["check"]);
    assert_eq!(code, 0, "{v}");
}

#[test]
fn edit_refuses_closed_documents() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Old idea");
    let (code, _) = r.json(&["set", &id, "status=rejected", "--write"]);
    assert_eq!(code, 0);
    let (code, v) = r.json(&["edit", &id, "--append", "--text", "more", "--write"]);
    assert_ne!(code, 0, "{v}");
}

#[test]
fn fix_i2_keeps_a_shared_alias_on_the_picked_document() {
    let r = common::repo();
    let a = r.new_doc("adr", "services/shop", "First readme");
    let b = r.new_doc("adr", "services/shop", "Second readme");
    for id in [&a, &b] {
        let (code, v) = r.json(&["set", id, "aliases+=README", "--write"]);
        assert_eq!(code, 0, "{v}");
    }
    let (_, v) = r.json(&["check", "--rule", "I2"]);
    assert_eq!(v["warnings"], 2, "{v}");
    let (_, v) = r.json(&["show", &a]);
    let a_path = v["path"].as_str().unwrap().to_string();
    let (code, v) = r.json(&["fix", "--rule", "I2", "--pick", &a, "--write", &format!("{a_path}:1")]);
    assert_eq!(code, 0, "{v}");
    let (_, v) = r.json(&["check", "--rule", "I2"]);
    assert_eq!(v["warnings"], 0, "{v}");
    let (_, v) = r.json(&["resolve", "README"]);
    assert_eq!(v["id"], a.as_str(), "{v}");
}

#[test]
fn edit_adds_a_new_section_after_a_named_one() {
    let r = common::repo();
    let id = r.new_doc("plan", "services/shop", "Checkout v2");
    assert_eq!(r.json(&["edit", &id, "--text=## Goal\n\nShip it.\n\n## Record\n\n- start\n", "--write"]).0, 0);
    let (code, v) = r.json(&["edit", &id, "--new-section", "Risks", "--after", "Goal", "--text=- none yet", "--write"]);
    assert_eq!(code, 0, "{v}");
    let path = r.json(&["resolve", &id]).1["path"].as_str().unwrap().to_string();
    let text = r.read(&path);
    assert!(text.contains("Ship it.\n\n## Risks\n\n- none yet\n\n## Record"), "{text}");
    let (code, _) = r.json(&["edit", &id, "--new-section", "Risks", "--text", "x"]);
    assert_ne!(code, 0, "a second Risks section is refused");
}

#[test]
fn edit_changes_one_table_row_by_its_key() {
    let r = common::repo();
    let id = r.new_doc("plan", "services/shop", "Checkout v2");
    let table = "## Tasks\n\n| Id | Status | Note |\n|----|--------|------|\n| A1 | next | first |\n| A2 | waiting | |\n\n## Record\n\n- start\n";
    assert_eq!(r.json(&["edit", &id, &format!("--text={table}"), "--write"]).0, 0);
    let (code, v) = r.json(&["edit", &id, "--section", "Tasks", "--row", "A1", "--cell", "Status=done", "--write"]);
    assert_eq!(code, 0, "{v}");
    let (code, v) = r.json(&["edit", &id, "--row", "A3", "--append", "--text=A3 | next | added", "--write"]);
    assert_eq!(code, 0, "{v}");
    let path = r.json(&["resolve", &id]).1["path"].as_str().unwrap().to_string();
    let text = r.read(&path);
    assert!(text.contains("| A1 | done | first |\n| A2 | waiting | |\n| A3 | next | added |\n\n## Record"), "{text}");
    let (code, v) = r.json(&["edit", &id, "--row", "A9", "--cell", "Status=done"]);
    assert_ne!(code, 0, "a missing row without --append is refused: {v}");
    let (code, _) = r.json(&["edit", &id, "--row", "A1", "--cell", "Owner=me"]);
    assert_ne!(code, 0, "an unknown column is refused");
}

#[test]
fn change_notes_collect_until_a_release_archives_them() {
    let r = common::repo();
    let (code, v) =
        r.json(&["new", "change", "--governs", "services/shop/src", "--title", "Cart keeps coupons", "--write"]);
    assert_eq!(code, 0, "{v}");
    let id = v["id"].as_str().unwrap().to_string();
    assert!(id.starts_with("change:"), "{v}");
    assert_eq!(r.json(&["set", &id, "kind=fixed", "--write"]).0, 0);
    let (_, v) = r.json(&["find", "--type", "change", "--status", "active"]);
    assert!(v.to_string().contains(&id), "pending notes are the active ones: {v}");
    assert_eq!(r.json(&["set", &id, "status=archived", "release=0.3.0", "--write"]).0, 0);
    let (_, v) = r.json(&["find", "--type", "change", "--status", "active"]);
    assert!(!v.to_string().contains(&id), "{v}");
    let (code, v) = r.json(&["check"]);
    assert_eq!(code, 0, "kind and release are known keys: {v}");
}

#[test]
fn b2_new_from_a_draft_supersedes_it_once_the_successor_is_active() {
    let r = common::repo();
    let old = r.new_doc("adr", "services/shop/src", "Cart rounding");
    let (code, v) =
        r.json(&["new", "adr", "--governs", "services/shop/src", "--title", "New rounding", "--from", &old, "--write"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(r.json(&["show", &old]).1["frontmatter"]["status"], "draft");
    let new = v["id"].as_str().unwrap().to_string();
    assert_eq!(r.json(&["set", &new, "status=active", "--write"]).0, 0);
    assert_eq!(r.json(&["show", &old]).1["frontmatter"]["status"], "superseded");
    assert_eq!(r.json(&["check", "--rule", "T1"]).1["errors"], 0);
    let other = r.new_doc("adr", "services/shop/src", "Tax rounding");
    let path = r.json(&["resolve", &other]).1["path"].as_str().unwrap().to_string();
    r.write(&path, &r.read(&path).replace("status: draft", "status: accepted"));
    let (_, text) = r.text(&["check"]);
    assert!(text.contains("for 'accepted' use 'active'"), "{text}");
}

#[test]
fn rerun_hint_from_a_preview_keeps_the_write() {
    let r = common::repo();
    r.write("spec.toml", &(r.read("spec.toml").replacen("[forbid]\n", "[forbid]\npaths = [\"**/.spec\"]\n", 1)));
    if !r.read("spec.toml").contains("**/.spec") {
        r.write("spec.toml", &(r.read("spec.toml") + "[forbid]\npaths = [\"**/.spec\"]\n"));
    }
    let base = ["new", "adr", "--governs", "services/shop", "--title", "Use files"];
    let (code, v) = r.json(&base);
    assert_ne!(code, 0, "{v}");
    let next = v["next"].as_str().unwrap_or_default().to_string();
    assert!(next.ends_with(" --local --write"), "{next}");
    let (_, v) = r.json(&[&base[..], &["--dry-run"]].concat());
    let dry = v["next"].as_str().unwrap_or_default();
    assert!(dry.ends_with(" --local") && !dry.contains("--write"), "{dry}");
    let (code, v) = r.json(&[&base[..], &["--local", "--write"]].concat());
    assert_eq!(code, 0, "following the hint writes: {v}");
    assert_eq!(v["written"], true, "{v}");
}

#[test]
fn new_refuses_a_forbidden_path_and_suggests_local() {
    let r = common::repo();
    r.write("spec.toml", &(r.read("spec.toml").replacen("[forbid]\n", "[forbid]\npaths = [\"**/.spec\"]\n", 1)));
    if !r.read("spec.toml").contains("**/.spec") {
        r.write("spec.toml", &(r.read("spec.toml") + "[forbid]\npaths = [\"**/.spec\"]\n"));
    }
    let (code, v) = r.json(&["new", "adr", "--governs", "services/shop", "--title", "Use files", "--write"]);
    assert_ne!(code, 0, "{v}");
    let e = v["error"].as_str().unwrap();
    assert!(e.contains("[forbid]"), "{e}");
    let next = v["next"].as_str().unwrap();
    assert!(next.starts_with("spec new adr ") && next.ends_with(" --local"), "{next}");
    assert!(!r.path().join("services/shop/.spec").exists(), "nothing written");
    let (code, v) = r.json(&["new", "adr", "--governs", "services/shop", "--title", "Use files", "--local", "--write"]);
    assert_eq!(code, 0, "{v}");
}

#[test]
fn set_rejecting_a_status_names_the_allowed_ones_and_the_next_command() {
    let r = common::repo();
    let id = r.new_doc("plan", "services/shop", "Checkout");
    assert_eq!(r.json(&["set", &id, "status=active", "--write"]).0, 0);
    let (code, v) = r.json(&["set", &id, "status=done", "--write"]);
    assert_ne!(code, 0);
    let e = v["error"].as_str().unwrap();
    assert!(e.contains("allowed:"), "{e}");
    assert_eq!(v["next"], format!("spec set {id} status=archived --write"));
}

#[test]
fn archiving_a_plan_with_open_tasks_is_refused_unless_forced() {
    let r = common::repo();
    let id = r.new_doc("plan", "services/shop", "Checkout");
    assert_eq!(r.json(&["set", &id, "status=active", "--write"]).0, 0);
    let (_, d) = r.json(&["show", &id]);
    let path = d["path"].as_str().unwrap().to_string();
    r.write(&path, &(r.read(&path) + "\n- [x] cart\n- [ ] payment\n"));
    // An explicit preview reports the open task and exits 0.
    let (code, v) = r.json(&["set", &id, "status=archived", "--dry-run"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["open_tasks"].as_array().unwrap().len(), 1, "{v}");
    let (code, v) = r.json(&["set", &id, "status=archived", "--write"]);
    assert_eq!(code, 4, "{v}");
    assert!(v["open_tasks"][0].as_str().unwrap().ends_with(": payment"), "{v}");
    assert!(v["next"].as_str().unwrap().contains("--force"), "{v}");
    assert!(r.read(&path).contains("status: active"), "nothing written");
    let (code, v) = r.json(&["set", &id, "status=archived", "--write", "--force"]);
    assert_eq!(code, 0, "{v}");
    assert!(r.read(&path).contains("status: archived"));
}

#[test]
fn mv_refuses_a_forbidden_target_before_writing() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Use files");
    r.write("spec.toml", &(r.read("spec.toml") + "[forbid]\npaths = [\"services/blog/.spec\"]\n"));
    let (code, v) = r.json(&["mv", &id, "--owner", "services/blog", "--write"]);
    assert_ne!(code, 0, "{v}");
    let e = v["error"].as_str().unwrap();
    assert!(e.contains("matches the forbidden path services/blog/.spec in spec.toml [forbid]"), "{e}");
    assert!(e.contains("--local"), "{e}");
    assert!(!r.path().join("services/blog/.spec").exists(), "nothing written");
}

#[test]
fn set_with_no_matching_finished_status_names_only_the_allowed_ones() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Use files");
    let (code, v) = r.json(&["set", &id, "status=done", "--write"]);
    assert_ne!(code, 0);
    let e = v["error"].as_str().unwrap();
    assert!(e.contains("allowed:") && !e.contains("next:"), "{e}");
    assert!(!v["next"].as_str().unwrap_or_default().contains("status="), "{v}");
}

#[test]
fn superseding_without_a_successor_points_to_new_from() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Use files");
    assert_eq!(r.json(&["set", &id, "status=active", "--write"]).0, 0);
    let (code, v) = r.json(&["set", &id, "status=superseded", "--write"]);
    assert_ne!(code, 0);
    assert!(v["next"].as_str().unwrap().starts_with(&format!("spec new adr --from {id} --title ")), "{v}");
    let (code, v) = r.json(&["new", "adr", "--from", &id, "--title", "Use a database", "--write"]);
    assert_eq!(code, 0, "{v}");
}

#[test]
fn set_warns_about_an_unknown_key_at_once() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Use files");
    let (code, v) = r.json(&["set", &id, "colour=blue", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert!(v["warnings"][0].as_str().unwrap().contains("unknown key 'colour'"), "{v}");
    assert!(v["next"].as_str().unwrap().contains("--rule S1"), "{v}");
}

#[test]
fn a_mostly_non_ascii_title_takes_its_slug_from_governs_or_asks_for_one() {
    let r = common::repo();
    let (code, v) =
        r.json(&["new", "adr", "--governs", "services/shop/src/cart", "--title", "결제는 PG 하나만 쓴다", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert!(v["path"].as_str().unwrap().ends_with("-cart.adr.md"), "{v}");
    assert!(v["slug_note"].as_str().unwrap().contains("--slug"), "{v}");
    let (code, v) = r.json(&["new", "adr", "--title", "결제", "--write"]);
    assert_ne!(code, 0);
    assert!(v["error"].as_str().unwrap().contains("--slug"), "{v}");
}

#[test]
fn the_first_local_document_keeps_the_local_directory_out_of_git() {
    let r = common::repo();
    let (code, v) = r.json(&["new", "plan", "--title", "Try it", "--local", "--write"]);
    assert_eq!(code, 0, "{v}");
    let status =
        std::process::Command::new("git").arg("-C").arg(r.path()).args(["status", "--porcelain"]).output().unwrap();
    assert!(!String::from_utf8_lossy(&status.stdout).contains(".spec.local"), "{v}");
    let (_, v) = r.json(&["new", "plan", "--title", "Again", "--local", "--write"]);
    assert!(v.get("ignored").is_none(), "only once: {v}");
}

#[test]
fn new_takes_the_title_as_a_positional_argument() {
    let r = common::repo();
    let (code, v) = r.json(&["new", "adr", "Use files", "--governs", "services/shop", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert!(v["path"].as_str().unwrap().ends_with("-use-files.adr.md"), "{v}");
    let (code, v) = r.json(&["new", "adr", "--write"]);
    assert_ne!(code, 0);
    assert!(v["next"].as_str().unwrap().starts_with("spec new adr --title '"), "{v}");
}

#[test]
fn a_mostly_non_ascii_title_points_at_slug() {
    let r = common::repo();
    let (code, v) =
        r.json(&["new", "adr", "--title", "결제는 PG 하나만 쓴다", "--governs", "services/shop", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert!(v["slug_note"].as_str().is_some(), "{v}");
    assert!(v["next"].as_str().unwrap().contains("--slug"), "{v}");
}

#[test]
fn mv_tier_promotes_a_local_document() {
    let r = common::repo();
    let (code, v) = r.json(&["new", "sot", "Notes", "--local", "--write"]);
    assert_eq!(code, 0, "{v}");
    let id = v["id"].as_str().unwrap().to_string();
    assert!(v["path"].as_str().unwrap().starts_with(".spec.local/"), "{v}");
    let (code, v) = r.json(&["mv", &id, "--tier", "committed", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert!(!v["path"].as_str().unwrap().contains(".spec.local"), "{v}");
    let (_, v) = r.json(&["show", &id]);
    assert!(!v["path"].as_str().unwrap().contains(".spec.local"), "{v}");
    assert_eq!(r.json(&["check"]).0, 0);
}

#[test]
fn mv_renames_the_slug_of_a_timestamp_keyed_document() {
    let r = common::repo();
    let id = r.new_doc("plan", "services/shop", "Checkout");
    let (code, v) = r.json(&["mv", &id, "--slug", "pay-flow", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["to"], id.as_str(), "{v}");
    assert!(v["path"].as_str().unwrap().ends_with("-pay-flow.plan.md"), "{v}");
}

#[test]
fn mv_lists_plain_text_paths_that_would_break() {
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart");
    let (_, v) = r.json(&["show", &id]);
    let old = v["path"].as_str().unwrap().to_string();
    r.write("NOTES.md", &format!("intro\nsee {old}\n"));
    r.git(&["add", "-A"]);
    r.git(&["commit", "-qm", "x"]);
    let (_, v) = r.json(&["mv", &id, "--slug", "basket"]);
    assert!(v.get("cited_paths").is_none(), "off by default: {v}");
    let (code, v) = r.json(&["mv", &id, "--slug", "basket", "--cited-paths", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["cited_paths"], 1, "{v}");
    assert_eq!(v["items"][0]["path"], "NOTES.md:2", "{v}");
    assert!(v["next"].as_str().unwrap().starts_with("replace "), "{v}");
}

#[test]
fn new_warns_about_a_repeated_title() {
    let r = common::repo();
    let first = r.new_doc("plan", "services/shop", "Second plan");
    let (code, v) = r.json(&["new", "plan", "--governs", "services/shop", "--title", "Second plan", "--dry-run"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["same_title"], serde_json::json!([first]), "{v}");
}

#[test]
fn mv_cited_paths_reads_staged_files() {
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart");
    let (_, v) = r.json(&["show", &id]);
    let old = v["path"].as_str().unwrap().to_string();
    r.git(&["add", "-A"]);
    r.git(&["commit", "-qm", "x"]);
    r.write("NOTES.md", &format!("see {old}\n"));
    r.git(&["add", "NOTES.md"]);
    let (_, v) = r.json(&["mv", &id, "--slug", "basket", "--cited-paths"]);
    assert_eq!(v["cited_paths"], 1, "staged, not committed: {v}");
}

#[test]
fn new_without_ascii_words_suggests_the_full_command() {
    let r = common::repo();
    let (code, v) = r.json(&["new", "sot", "--title", "장바구니", "--write"]);
    assert_eq!(code, 2, "{v}");
    let next = v["next"].as_str().unwrap();
    assert!(next.starts_with("spec new sot --title") && next.ends_with("--slug my-slug"), "{v}");
}

#[test]
fn required_sections_are_in_new_documents_and_a_missing_one_names_the_command() {
    let r = common::repo();
    let toml = std::fs::read_to_string(r.path().join("spec.toml")).unwrap_or_default();
    r.write("spec.toml", &format!("{toml}\n[types.adr]\nsections = [\"Decision\", \"Risks\"]\n"));
    assert_eq!(r.text(&["new", "adr", "Pick a queue", "--write"]).0, 0);
    let (_, l) = r.json(&["list", "--type", "adr"]);
    let id = l["items"][0]["id"].as_str().unwrap().to_string();
    let (_, d) = r.json(&["show", &id]);
    assert!(d["outline"].to_string().contains("Risks"), "{d}");
    let (_, c) = r.json(&["check"]);
    assert!(c["by_rule"]["S7"].is_null(), "{c}");

    r.write("spec.toml", &format!("{toml}\n[types.adr]\nsections = [\"Decision\", \"Rollout plan\"]\n"));
    let (_, out) = r.text(&["check"]);
    assert!(out.contains(&format!("next: spec edit {id} --new-section 'Rollout plan' --text \"…\" --write")), "{out}");
    let (code, out) = r.text(&["edit", &id, "--section", "Rollout plan", "--text", "x"]);
    assert_ne!(code, 0);
    assert!(out.contains(&format!("next: spec edit {id} --new-section 'Rollout plan' --text=x --write")), "{out}");
}

#[test]
fn a_missing_section_error_leaves_out_the_title() {
    let r = common::repo();
    let id = r.new_doc("plan", "services/shop", "Payment rollout");
    let (code, v) = r.json(&["edit", &id, "--section", "Nope", "--text", "x"]);
    assert_ne!(code, 0, "{v}");
    let s = v.to_string();
    assert!(s.contains("sections: ") && !s.contains("sections: Payment rollout"), "{s}");
}

#[test]
fn c117_a_set_that_changes_nothing_writes_nothing() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Use files");
    r.commit("doc");
    let (_, d) = r.json(&["show", &id]);
    let path = d["path"].as_str().unwrap().to_string();
    let before = r.read(&path);
    let (code, v) = r.json(&["set", &id, "title=Use files", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(
        (v["written"].clone(), v["changed"].clone()),
        (serde_json::json!(false), serde_json::json!(false)),
        "{v}"
    );
    assert!(v.get("undo").is_none(), "{v}");
    assert_eq!(r.read(&path), before, "updated is not bumped");
}

#[test]
fn c165_draft_refusal_and_missing_title_keep_from() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Use files");
    let (code, v) = r.json(&["set", &id, "status=superseded", "--write"]);
    assert_eq!(code, 2, "{v}");
    let e = v.to_string();
    assert!(e.contains(&format!("spec new adr --from {id} --title 'Use files' --write")), "{e}");
    let (code, v) = r.json(&["new", "adr", "--from", &id, "--write"]);
    assert_eq!(code, 2, "{v}");
    let next = v["next"].as_str().unwrap();
    assert!(next.contains(&format!("--from {id}")) && next.contains("--write") && next.contains("--title"), "{v}");
}

#[test]
fn c167_fix_refuses_an_unknown_or_lower_case_rule() {
    let r = common::repo();
    let (code, v) = r.json(&["fix", "--rule", "ZZ9", "--write"]);
    assert_eq!(code, 2, "{v}");
    assert!(v["error"].as_str().unwrap().contains("no such rule"), "{v}");
    let (code, v) = r.json(&["fix", "--rule", "s1"]);
    assert_eq!(code, 2, "{v}");
    assert!(v["error"].as_str().unwrap().contains("upper case: --rule S1"), "{v}");
    assert_eq!(r.json(&["fix", "--rule", "S1"]).0, 0);
}

#[test]
fn c168_fix_write_with_nothing_to_fix_is_no_change() {
    let r = common::repo();
    let (code, v) = r.json(&["fix", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["written"], false, "{v}");
    assert!(v.get("undo").is_none_or(|u| u.as_str().is_some_and(|s| !s.is_empty())), "{v}");
    let (code, text) = r.text(&["fix", "--write"]);
    assert_eq!(code, 0, "{text}");
    assert!(text.contains("no change") && !text.contains("undo:") && !text.contains("written"), "{text}");
}

#[test]
fn c178_new_with_an_unknown_option_points_at_help_not_the_same_error() {
    let r = common::repo();
    for bad in [["--tier", "local"], ["--bogus", "x"]] {
        let mut args = vec!["new", "sot", "--write"];
        args.extend(bad);
        args.extend(["--governs", "src/x", "--title", "T", "--write"]);
        let (code, v) = r.json(&args);
        assert_eq!(code, 2, "{v}");
        let next = v["next"].as_str().unwrap_or_default();
        assert!(!next.contains(bad[0]), "the rejected option is not carried over: {v}");
        assert_eq!(next, "spec help new", "{v}");
    }
    // A known option still carries over (C165).
    let (_, v) = r.json(&["new", "adr", "--write"]);
    assert!(v["next"].as_str().unwrap().contains("--write"), "{v}");
}

#[test]
fn c180_mv_rewrites_reference_angle_and_encoded_links_but_not_code() {
    let r = common::repo();
    let a = r.new_doc("sot", "services/shop", "Cart");
    let b = r.new_doc("sot", "services/shop", "Pay");
    let (_, s) = r.json(&["show", &b]);
    let pb = s["path"].as_str().unwrap().to_string();
    let text = r.read(&pb);
    r.write(
        &pb,
        &format!(
            "{text}\n[x][c] [a](<cart.sot.md#verification>) [e](cart%2Esot.md)\n\n[c]: ./cart.sot.md \"Cart\"\n\n`[k](cart.sot.md)`\n"
        ),
    );
    r.commit("links");
    let (code, v) = r.json(&["mv", &a, "--slug", "basket", "--write"]);
    assert_eq!(code, 0, "{v}");
    let t = r.read(&pb);
    assert!(t.contains("[c]: basket.sot.md \"Cart\""), "{t}");
    assert!(t.contains("[a](<basket.sot.md#verification>)") && t.contains("[e](basket.sot.md)"), "{t}");
    assert_eq!(t.matches("cart.sot.md").count(), 1, "only the code span keeps the old name: {t}");
}

#[test]
fn c182_duplicate_row_keys_point_at_the_section_or_at_naming_one() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Use files");
    let (_, s) = r.json(&["show", &id]);
    let p = s["path"].as_str().unwrap().to_string();
    let table = "| Id | Status |\n|---|---|\n| A1 | next |\n| A1 | dup |\n";
    r.write(
        &p,
        &format!(
            "{}\n## Steps\n\n{table}\n## Other\n\n| Id | Status |\n|---|---|\n| B1 | x |\n| A2 | y |\n",
            r.read(&p)
        ),
    );
    let lines: Vec<usize> =
        r.read(&p).lines().enumerate().filter(|(_, l)| l.starts_with("| A1")).map(|(i, _)| i + 1).collect();
    for extra in [&["--section", "Steps"][..], &[][..]] {
        let (code, v) = r.json(&[&["edit", id.as_str(), "--row", "A1", "--cell", "Status=done"][..], extra].concat());
        assert_eq!(code, 2, "{v}");
        let err = v["error"].as_str().unwrap();
        assert!(err.contains(&format!("lines {}, {}", lines[0], lines[1])) && !err.contains("--section"), "{v}");
        assert_eq!(v["next"], format!("spec edit {id} --section Steps"), "{v}");
    }
    // Spread over two sections: naming one is the way out.
    r.write(&p, &r.read(&p).replace("| A2 | y |", "| A1 | y |"));
    let (code, v) = r.json(&["edit", &id, "--row", "A1", "--cell", "Status=done"]);
    assert_eq!(code, 2, "{v}");
    assert!(v["error"].as_str().unwrap().contains("name the section with --section"), "{v}");
}

#[test]
fn c183_a_write_over_uncommitted_edits_keeps_a_copy_the_undo_restores() {
    let r = common::repo();
    let dirty = r.new_doc("adr", "services/shop", "Use files");
    let clean = r.new_doc("adr", "services/shop", "Use logs");
    r.commit("docs");
    let (_, s) = r.json(&["show", &dirty]);
    let p = s["path"].as_str().unwrap().to_string();
    r.write(&p, &(r.read(&p) + "\nmy own edit\n"));
    let mine = r.read(&p);
    let (_, v) = r.json(&["edit", &dirty, "--append", "--text", "spec line"]);
    assert_eq!(v["uncommitted"][0], p.as_str(), "the preview says so: {v}");
    let (code, v) = r.json(&["edit", &dirty, "--append", "--text", "spec line", "--write"]);
    assert_eq!(code, 0, "{v}");
    let undo = v["undo"].as_str().unwrap().to_string();
    assert!(undo.starts_with("cp -Rp .specfold/trash/") && !undo.contains("git stash"), "{undo}");
    let ok = std::process::Command::new("sh").arg("-c").arg(&undo).current_dir(r.path()).status().unwrap();
    assert!(ok.success() && r.read(&p) == mine, "the undo gives back the edited file, not HEAD");
    let (_, v) = r.json(&["edit", &clean, "--append", "--text", "x", "--write"]);
    assert!(v["undo"].as_str().unwrap().starts_with("git restore -- "), "a clean file comes back from git: {v}");
}

#[test]
fn c187_writes_in_one_second_never_share_or_overwrite_a_trash_folder() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Use files");
    r.commit("docs");
    let (_, s) = r.json(&["show", &id]);
    let p = s["path"].as_str().unwrap().to_string();
    r.write(&p, &(r.read(&p) + "\nmy own line\n"));
    let mine = r.read(&p);
    // Folders already taken for the next few seconds: the write must pick another name.
    let out = std::process::Command::new("date").args(["-u", "+%s"]).output().unwrap();
    let now: i64 = String::from_utf8_lossy(&out.stdout).trim().parse().unwrap();
    for t in now..now + 5 {
        let o =
            std::process::Command::new("date").args(["-u", "-d", &format!("@{t}"), "+%Y%m%d%H%M%S"]).output().unwrap();
        r.write(&format!(".specfold/trash/{}/keep.txt", String::from_utf8_lossy(&o.stdout).trim()), "old");
    }
    let undo = |text: &str| {
        let (code, v) = r.json(&["edit", &id, "--append", "--text", text, "--write"]);
        assert_eq!(code, 0, "{v}");
        v["undo"].as_str().unwrap().to_string()
    };
    let (first, second) = (undo("one"), undo("two"));
    assert_ne!(first, second, "each write has its own folder");
    let bin = first.trim_start_matches("cp -Rp ").trim_end_matches("/. .");
    assert!(bin.contains('-'), "{bin}");
    assert_eq!(r.read(&format!("{bin}/{p}")), mine, "the first copy still holds the user's line");
    for d in std::fs::read_dir(r.path().join(".specfold/trash")).unwrap() {
        let k = d.unwrap().path().join("keep.txt");
        assert!(!k.exists() || std::fs::read_to_string(&k).unwrap() == "old");
    }
}

#[test]
fn c192_dir_into_a_subfolder_reads_paths_like_cd_and_its_undo_works() {
    let r = common::repo();
    let d = r.new_doc("sot", "services/shop/src", "Cart");
    r.commit("doc");
    let out = r.path().parent().unwrap().to_path_buf();
    let name = r.path().file_name().unwrap().to_str().unwrap().to_string();
    let sub = format!("{name}/services/shop");
    let run = |args: &[&str]| {
        let o = r.spec().current_dir(&out).arg("-C").arg(&sub).args(args).arg("--json").output().unwrap();
        (o.status.code().unwrap_or(-1), serde_json::from_slice::<serde_json::Value>(&o.stdout).unwrap_or_default())
    };
    let (_, v) = run(&["context", "src"]);
    assert_eq!(v["items"][0]["id"], d.as_str(), "{v}");
    let (code, v) = run(&["new", "sot", "--governs", "src", "--title", "Dup", "--write"]);
    assert_eq!(code, 0, "{v}");
    let id = v["id"].as_str().unwrap().to_string();
    let (_, s) = run(&["show", &id]);
    assert!(s.to_string().contains("services/shop/src"), "governs is rebased to the root: {s}");
    let undo = v["undo"].as_str().unwrap().to_string();
    assert!(undo.starts_with(&format!("cd {name} && rm -rf -- ")), "{undo}");
    let st = std::process::Command::new("sh").arg("-c").arg(&undo).current_dir(&out).status().unwrap();
    assert!(st.success());
    assert_ne!(run(&["show", &id]).0, 0, "the undo removed the new file");
}

#[test]
fn c188_new_section_names_are_one_plain_line() {
    let r = common::repo();
    let id = r.new_doc("plan", "services/shop", "Ship");
    let before = r.json(&["show", &id]).1["path"].as_str().map(|p| r.read(p)).unwrap();
    for name in ["Risks\n\n## Goal", "", "   ", "## Bad"] {
        let (code, text) = r.text(&["edit", &id, "--new-section", name, "--text", "y", "--write"]);
        assert_eq!(code, 2, "{name:?}: {text}");
        assert!(text.contains("the section name") && text.contains("--new-section"), "{text}");
    }
    let after = r.json(&["show", &id]).1["path"].as_str().map(|p| r.read(p)).unwrap();
    assert_eq!(before, after);
}

#[test]
fn c196_audit_records_take_a_slug_for_a_title_without_ascii_words() {
    let r = common::repo();
    r.write("review.md", "# Review\n\nall good\n");
    r.commit("review");
    let (code, text) = r.text(&["audit", "record", "--from", "review.md", "--title", "분기 감사"]);
    assert_eq!(code, 2, "{text}");
    assert!(text.contains("next: spec audit record --from review.md --title '분기 감사' --slug my-slug"), "{text}");
    let (code, v) =
        r.json(&["audit", "record", "--from", "review.md", "--title", "분기 감사", "--slug", "q-audit", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert!(v["path"].as_str().unwrap().contains("q-audit"), "{v}");
    assert!(v["undo"].as_str().unwrap().starts_with("rm -rf -- "), "{v}");
    let (code, v) = r.json(&["audit", "--record", "--title", "분기 감사", "--slug", "q-two"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(r.json(&["audit", "--record", "--title", "x", "--slug", "Bad_Slug"]).0, 2);
}

fn run_undo(r: &common::Repo, undo: &str) {
    let st = std::process::Command::new("sh").arg("-c").arg(undo).current_dir(r.path()).status().unwrap();
    assert!(st.success(), "{undo}");
}

#[test]
fn c197_c209_undo_reaches_ignored_and_never_committed_documents() {
    let r = common::repo();
    r.write(".gitignore", ".spec.local/\n.specfold/\n");
    r.commit("ignore");
    // C197: a new document in the ignored local tier.
    let (code, v) = r.json(&["new", "guide", "--title", "Priv two", "--local", "--write"]);
    assert_eq!(code, 0, "{v}");
    let path = v["path"].as_str().unwrap().to_string();
    let undo = v["undo"].as_str().unwrap().to_string();
    assert!(undo.starts_with("rm -f -- "), "{undo}");
    run_undo(&r, &undo);
    assert!(!r.path().join(&path).exists());
    // C197: a committed document moved into the local tier comes back.
    let (code, v) = r.json(&["new", "guide", "--title", "Keep me", "--write"]);
    assert_eq!(code, 0, "{v}");
    let kept = v["path"].as_str().unwrap().to_string();
    r.commit("keep");
    let (code, v) = r.json(&["mv", "guide:ws/keep-me", "--tier", "local", "--write"]);
    assert_eq!(code, 0, "{v}");
    run_undo(&r, v["undo"].as_str().unwrap());
    assert!(r.path().join(&kept).is_file());
    // C209: set on a document never committed keeps it, with its old text.
    let (code, v) = r.json(&["new", "guide", "--title", "Checkout Flow", "--write"]);
    assert_eq!(code, 0, "{v}");
    let fresh = v["path"].as_str().unwrap().to_string();
    let before = r.read(&fresh);
    let (code, v) = r.json(&["set", "guide:ws/checkout-flow", "tags=retry", "--write"]);
    assert_eq!(code, 0, "{v}");
    let undo = v["undo"].as_str().unwrap().to_string();
    assert!(undo.contains(".specfold/trash/"), "{undo}");
    run_undo(&r, &undo);
    assert_eq!(r.read(&fresh), before);
}

#[test]
fn c208_edit_refuses_text_with_an_open_code_fence() {
    let r = common::repo();
    let d = r.new_doc("guide", "services/shop", "Deploy");
    let before = r.json(&["show", &d, "--section", "Steps"]).0;
    assert_eq!(before, 0);
    for text in ["```\ncode", "~~~\nsnippet\n```"] {
        let (code, t) = r.text(&["edit", &d, "--section", "Overview", "--text", text, "--write"]);
        assert_eq!(code, 2, "{t}");
        assert!(t.contains("never closes"), "{t}");
    }
    let (code, v) = r.json(&["edit", &d, "--section", "Overview", "--text", "```\ncode\n```", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(r.json(&["show", &d, "--section", "Steps"]).0, 0);
}

#[test]
fn c206_concurrent_new_runs_get_distinct_timestamp_ids() {
    let r = common::repo();
    let ids: std::collections::BTreeSet<String> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..4)
            .map(|n| {
                let r = &r;
                sc.spawn(move || {
                    let title = format!("Para {n}");
                    let (code, v) = r.json(&["new", "adr", "--governs", "services/shop", "--title", &title, "--write"]);
                    assert_eq!(code, 0, "{v}");
                    v["id"].as_str().unwrap().to_string()
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    assert_eq!(ids.len(), 4, "{ids:?}");
    let (_, v) = r.json(&["check"]);
    assert_eq!(v["errors"], 0, "{v}");
}

#[test]
fn c214_fix_refuses_targets_check_would_refuse() {
    let r = common::repo();
    let id = r.new_doc("guide", "services/shop", "Pay retry");
    let path = r.json(&["resolve", &id]).1["path"].as_str().unwrap().to_string();
    let dir = std::path::Path::new(&path).parent().unwrap().to_string_lossy().to_string();
    let typo = path.replace(".md", ".mdx");
    let bad_line = format!("{path}:abc");
    // B2: a folder with no documents is refused; one with documents names them, as in check.
    for t in ["nope.md", typo.as_str(), "services/blog", bad_line.as_str()] {
        let (code, v) = r.json(&["fix", t]);
        assert_eq!(code, 2, "{t}: {v}");
    }
    let line = format!("{path}:1");
    for t in [path.as_str(), id.as_str(), line.as_str(), dir.as_str()] {
        assert_eq!(r.json(&["fix", t]).0, 0, "{t}");
    }
}

#[test]
fn c224_mv_counts_a_referring_document_once() {
    let r = common::repo();
    let sot = r.new_doc("sot", "services/shop", "Cart");
    let plan = r.new_doc("plan", "services/shop", "Ship");
    let rel = format!("rel.depends_on+={sot}");
    assert_eq!(r.json(&["set", &plan, &rel, "--write"]).0, 0);
    let plan_path = r.json(&["resolve", &plan]).1["path"].as_str().unwrap().to_string();
    let text = r.read(&plan_path) + &format!("\nSee [[{sot}]].\n");
    r.write(&plan_path, &text);
    for extra in [&[][..], &["--write"][..]] {
        let mut args = vec!["mv", sot.as_str(), "--slug", "basket"];
        args.extend_from_slice(extra);
        let (code, v) = r.json(&args);
        assert_eq!(code, 0, "{v}");
        assert_eq!(v["references"], 1, "{extra:?}: {v}");
    }
}

#[test]
fn c223_set_checks_and_keeps_updated() {
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart");
    let (_, d) = r.json(&["show", &id]);
    let was = d["frontmatter"]["updated"].clone();
    for bad in ["updated=notadate", "updated=2026-13-45", "updated="] {
        for write in [false, true] {
            let mut args = vec!["set", id.as_str(), bad, "--write"];
            if write {
                args.push("--write");
            }
            let (code, v) = r.json(&args);
            assert_eq!(code, 2, "{bad} write={write}: {v}");
        }
    }
    assert_eq!(r.json(&["show", &id]).1["frontmatter"]["updated"], was);
    for good in ["updated=2026-10-30", "updated=2026-10-30T02:21:00Z", "updated=today"] {
        let (code, v) = r.json(&["set", &id, good, "--write"]);
        assert_eq!(code, 0, "{good}: {v}");
        assert!(v["next"].as_str().unwrap_or_default().contains("--rule S5"), "{good}: {v}");
    }
}

#[test]
fn c226_ambiguous_section_names_file_lines() {
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart");
    let path = r.json(&["resolve", &id]).1["path"].as_str().unwrap().to_string();
    r.write(&path, &(r.read(&path) + "\n## Steps\na\n\n## Steps\nb\n"));
    let text = r.read(&path);
    let want: Vec<String> = text
        .lines()
        .enumerate()
        .filter(|(_, l)| *l == "## Steps")
        .map(|(i, _)| format!("line {} 'Steps'", i + 1))
        .collect();
    assert_eq!(want.len(), 2);
    for args in [
        vec!["edit", id.as_str(), "--section", "Steps", "--text", "x"],
        vec!["edit", id.as_str(), "--section", "Steps", "--row", "1", "--text", "x"],
        vec!["show", id.as_str(), "--section", "Steps"],
    ] {
        let (code, t) = r.text(&args);
        assert_ne!(code, 0, "{args:?}: {t}");
        if t.contains("matches 2 headings") {
            assert!(want.iter().all(|w| t.contains(w.as_str())), "{args:?}: {want:?} in {t}");
        }
    }
    let (_, t) = r.text(&["edit", id.as_str(), "--section", "Steps", "--text", "x"]);
    assert!(t.contains(&want[1]), "{t}");
}

/// C233/C234: moving a committed bundle gives an undo that runs, and links to its attachments follow.
#[test]
fn c233_c234_bundle_mv_undo_runs_and_attachment_links_follow() {
    let r = common::repo();
    let design = r.new_doc("design", "services/shop", "Cart board");
    r.write("board.png", "png");
    r.write("flow.svg", "<svg/>");
    let (code, v) = r.json(&["attach", &design, "board.png", "--write"]);
    assert_eq!(code, 0, "{v}");
    let (code, v) = r.json(&["attach", &design, "flow.svg", "--into", "diagrams", "--write"]);
    assert_eq!(code, 0, "{v}");
    let (_, s) = r.json(&["show", &design]);
    let main = s["path"].as_str().unwrap().to_string();
    let dir = main.rsplit_once('/').unwrap().0.to_string();
    let sot = r.new_doc("sot", "services/shop", "Cart rules");
    let (_, s) = r.json(&["show", &sot]);
    let sp = s["path"].as_str().unwrap().to_string();
    let rel = |p: &str| {
        let up = "../".repeat(sp.matches('/').count());
        format!("{up}{p}")
    };
    let links = format!(
        "\nBoard: ![b]({})\nDoc: [d]({})\nSvg: [s]({})\n",
        rel(&format!("{dir}/board.png")),
        rel(&main),
        rel(&format!("{dir}/diagrams/flow.svg"))
    );
    r.write(&sp, &(r.read(&sp) + &links));
    r.commit("docs");
    let before = r.read(&sp);
    let (code, v) = r.json(&["mv", &design, "--slug", "basket", "--write"]);
    assert_eq!(code, 0, "{v}");
    let after = r.read(&sp);
    assert!(!after.contains(&format!("{dir}/")), "every link under the old bundle follows it: {after}");
    let (_, c) = r.json(&["check"]);
    assert_eq!(c["warnings"], 0, "no broken link left: {c}");
    let undo = v["undo"].as_str().unwrap().to_string();
    let out = std::process::Command::new("sh").arg("-c").arg(&undo).current_dir(r.path()).output().unwrap();
    assert!(out.status.success(), "{undo}: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(r.read(&sp), before, "the referrer is back");
    assert!(r.path().join(&main).is_file(), "the bundle is back at {main}");
    let st = std::process::Command::new("git").args(["status", "--porcelain"]).current_dir(r.path()).output().unwrap();
    assert!(st.stdout.is_empty(), "{}", String::from_utf8_lossy(&st.stdout));
}

#[test]
fn c248_first_local_doc_gets_an_rm_undo() {
    let r = common::repo();
    r.write(".gitignore", ".cache/\n");
    let (code, v) =
        r.json(&["new", "plan", "--governs", "services/shop", "--title", "First local", "--local", "--write"]);
    assert_eq!(code, 0, "{v}");
    let undo = v["undo"].as_str().unwrap();
    assert!(undo.starts_with("rm -f -- "), "{v}");
}

/// E3: mv leaves documents it does not need to change byte for byte, and in a referring
/// document changes only the reference, keeping quotes elsewhere.
#[test]
fn e3_mv_keeps_quotes_in_unrelated_and_referring_documents() {
    let r = common::repo();
    let sot = r.new_doc("sot", "services/shop", "Cart");
    let blog = r.new_doc("sot", "services/blog", "Blog rules");
    let plan = r.new_doc("plan", "services/shop", "Ship");
    assert_eq!(r.json(&["set", &plan, &format!("rel.depends_on+={sot}"), "--write"]).0, 0);
    let path = |id: &str| r.json(&["resolve", id]).1["path"].as_str().unwrap().to_string();
    let quote = |p: &str| {
        let t = r.read(p);
        let t = t.lines().map(|l| match l.strip_prefix("title: ") {
            Some(t) => format!("title: '{t}'"),
            None => l.to_string(),
        });
        r.write(p, &(t.collect::<Vec<_>>().join("\n") + "\n"));
    };
    let (bp, pp) = (path(&blog), path(&plan));
    quote(&bp);
    quote(&pp);
    r.commit("quoted");
    let before = (r.read(&bp), r.read(&pp));
    let (code, v) = r.json(&["mv", &sot, "--slug", "basket", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(r.read(&bp), before.0, "unrelated document rewritten");
    let new_id = v["to"].as_str().unwrap();
    assert_eq!(r.read(&pp), before.1.replace(sot.as_str(), new_id), "referring document changed beyond the reference");
}

/// E2: an undo never touches the stash every worktree shares: a created file is removed and a
/// committed file comes back from git, while someone else's stash entry stays where it was.
#[test]
fn e2_undo_leaves_the_shared_stash_alone() {
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart");
    r.commit("doc");
    r.write("services/blog/src/main.rs", "fn main() { other(); }\n");
    r.git(&["stash", "push", "-q", "--", "services/blog/src/main.rs"]);
    let stash = |r: &common::Repo| {
        let o = std::process::Command::new("git").args(["stash", "list"]).current_dir(r.path()).output().unwrap();
        String::from_utf8_lossy(&o.stdout).into_owned()
    };
    let before = stash(&r);
    assert_eq!(before.lines().count(), 1);
    let run = |undo: &str| {
        assert!(!undo.contains("stash"), "{undo}");
        let ok = std::process::Command::new("sh").args(["-c", undo]).current_dir(r.path()).status().unwrap();
        assert!(ok.success(), "{undo}");
    };
    let (code, v) = r.json(&["set", &id, "title=Basket", "--write"]);
    assert_eq!(code, 0, "{v}");
    run(v["undo"].as_str().unwrap());
    let (code, v) = r.json(&["new", "plan", "--governs", "services/shop", "--title", "Ship", "--write"]);
    assert_eq!(code, 0, "{v}");
    let made = v["path"].as_str().unwrap().to_string();
    run(v["undo"].as_str().unwrap());
    assert!(!r.path().join(&made).exists(), "{made}");
    let st = std::process::Command::new("git").args(["status", "--porcelain"]).current_dir(r.path()).output().unwrap();
    assert!(st.stdout.is_empty(), "{}", String::from_utf8_lossy(&st.stdout));
    assert_eq!(stash(&r), before);
}

#[test]
fn e4_set_help_and_schema_list_the_status_values() {
    let r = common::repo();
    let (code, help) = r.text(&["set", "--help"]);
    assert_eq!(code, 0);
    assert!(help.contains("adr: draft, active, superseded, rejected"), "{help}");
    assert!(help.contains("spec schema <type>"), "{help}");
    let (code, text) = r.text(&["schema", "adr"]);
    assert_eq!(code, 0);
    assert!(text.contains("statuses: draft -> active, superseded, rejected;"), "{text}");
    let (_, j) = r.json(&["schema", "adr"]);
    assert_eq!(j["status_moves"][0], "draft -> active, superseded, rejected", "{j}");
}

#[test]
fn e6_depends_on_takes_a_comma_list_and_a_file() {
    let r = common::repo();
    let a = r.new_doc("sot", "services/shop", "Cart");
    let b = r.new_doc("sot", "services/blog", "Posts");
    let (code, v) = r.json(&["new", "change", "--title", "Both", "--depends-on", &format!("{a},{b}"), "--write"]);
    assert_eq!(code, 0, "{v}");
    let text = r.read(v["path"].as_str().unwrap());
    assert!(text.contains(&a) && text.contains(&b), "{text}");
    r.write("deps.txt", &format!("# what this note updates\n{a}\n\n{b}  # blog\n{a}\n"));
    let (code, v) = r.json(&["new", "change", "--title", "From file", "--depends-on-file", "deps.txt", "--write"]);
    assert_eq!(code, 0, "{v}");
    let text = r.read(v["path"].as_str().unwrap());
    assert!(text.contains(&a) && text.contains(&b), "{text}");
    assert_eq!(text.matches(&a).count(), 1, "duplicates collapse: {text}");
    let (code, v) = r.json(&["new", "change", "--title", "Missing", "--depends-on-file", "nope.txt", "--write"]);
    assert_ne!(code, 0, "{v}");
}

#[test]
fn c262_write_from_a_subfolder_without_dir_gives_an_undo_that_works_there() {
    let r = common::repo();
    r.new_doc("sot", "services/shop/src", "Cart");
    r.commit("doc");
    let sub = r.path().join("services/shop");
    let o = r.spec().current_dir(&sub).args(["new", "plan", "--title", "Sub", "--write", "--json"]).output().unwrap();
    assert_eq!(o.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    let undo = v["undo"].as_str().unwrap().to_string();
    assert!(undo.starts_with("cd ../.. && rm -rf -- services/shop/.spec/plan/"), "the folder's owner: {undo}");
    let id = v["id"].as_str().unwrap().to_string();
    let st = std::process::Command::new("sh").arg("-c").arg(&undo).current_dir(&sub).status().unwrap();
    assert!(st.success());
    assert_ne!(r.spec().args(["show", &id]).output().unwrap().status.code(), Some(0), "the undo removed the file");
    let (_, root) = r.json(&["new", "plan", "--title", "Top", "--write"]);
    assert!(root["undo"].as_str().unwrap().starts_with("rm -rf -- "), "{root}");
}

#[test]
fn c122_mv_undo_restores_committed_and_uncommitted_documents() {
    for commit in [true, false] {
        let r = common::repo();
        let id = r.new_doc("adr", "services/shop", "Use files");
        if commit {
            r.commit("doc");
        }
        let (_, d) = r.json(&["show", &id]);
        let path = d["path"].as_str().unwrap().to_string();
        let text = r.read(&path);
        // A second later the move bumps `updated`, so the undo must bring the old text back.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let (code, v) = r.json(&["mv", &id, "--slug", "use-plain-files", "--write"]);
        assert_eq!(code, 0, "{v}");
        let undo = v["undo"].as_str().unwrap().to_string();
        let out = std::process::Command::new("sh").args(["-c", &undo]).current_dir(r.path()).output().unwrap();
        assert!(out.status.success(), "commit={commit} {undo}: {}", String::from_utf8_lossy(&out.stderr));
        assert_eq!(r.read(&path), text, "commit={commit}: {undo}");
        let (_, d) = r.json(&["show", &id]);
        assert_eq!(d["path"], path.as_str(), "commit={commit}");
    }
}

#[test]
fn c128_set_refuses_emptying_the_title() {
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart");
    let (code, v) = r.json(&["set", &id, "title=", "--write"]);
    assert_eq!(code, 2, "{v}");
    let (_, d) = r.json(&["show", &id]);
    assert_eq!(d["frontmatter"]["title"], "Cart", "{d}");
    assert_eq!(r.json(&["set", &id, "title=Basket", "--write"]).0, 0);
}

#[test]
fn c154_writes_keep_crlf_line_endings() {
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart");
    let path = common::doc_path(&r, &id);
    let crlf = r.read(&path).replace('\n', "\r\n");
    r.write(&path, &crlf);
    r.commit("crlf");
    assert_eq!(r.json(&["set", &id, "tags+=x", "--write"]).0, 0);
    assert_eq!(r.json(&["edit", &id, "--section", "Rule", "--text", "New rule.", "--write"]).0, 0);
    let text = r.read(&path);
    assert!(text.contains("New rule.") && text.contains("x"), "{text}");
    assert_eq!(text.matches('\n').count(), text.matches("\r\n").count(), "only CRLF: {text:?}");
}

#[test]
fn c139_an_id_held_by_two_files_is_refused() {
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart");
    let path = common::doc_path(&r, &id);
    let owner = id.split(':').nth(1).unwrap().split('/').next().unwrap();
    let local = format!(".spec.local/{owner}/sot/cart.sot.md");
    r.write(&local, &r.read(&path));
    r.write(".gitignore", ".spec.local/\n");
    r.commit("dup");
    let (code, v) = r.json(&["mv", &id, "--slug", "cart-local", "--write"]);
    assert_eq!(code, 2, "{v}");
    assert!(v.to_string().contains(&local), "{v}");
    assert!(r.path().join(&path).exists());
    let (_, v) = r.json(&["doctor"]);
    assert!(v.to_string().contains(&format!("spec mv {local}")), "{v}");
}

#[test]
fn c137_mv_rewrites_relative_markdown_links() {
    let r = common::repo();
    let a = r.new_doc("sot", "services/shop", "Cart");
    let b = r.new_doc("sot", "services/shop", "Pay");
    let pb = common::doc_path(&r, &b);
    let text = r.read(&pb);
    r.write(
        &pb,
        &format!(
            "{text}\nSee [cart](cart.sot.md) and [v](./cart.sot.md#verification) and [w](https://x.io/cart.sot.md).\n"
        ),
    );
    r.commit("links");
    let (code, v) = r.json(&["mv", &a, "--slug", "basket", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert!(v["next"].to_string().contains("L8"), "{v}");
    let t = r.read(&pb);
    assert!(t.contains("[cart](basket.sot.md)") && t.contains("[v](basket.sot.md#verification)"), "{t}");
    assert!(t.contains("https://x.io/cart.sot.md"), "{t}");
    let (_, v) = r.json(&["check", "--rule", "L8"]);
    assert!(!v.to_string().contains("cart.sot.md"), "{v}");
}

#[test]
fn c125_section_text_may_not_start_another_section() {
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart");
    for args in [
        &["edit", &id, "--section", "Rule", "--text", "x\n## Why\ndup", "--write"][..],
        &["edit", &id, "--new-section", "Risks", "--text", "- r\n# Top\n", "--write"],
    ] {
        let (code, v) = r.json(args);
        assert_eq!(code, 2, "{args:?}: {v}");
    }
    assert_eq!(
        r.json(&["edit", &id, "--section", "Rule", "--text", "x\n### Detail\n```\n## not a heading\n```", "--write"]).0,
        0
    );
}

#[test]
fn c130_governs_paths_must_be_inside_the_repository() {
    let r = common::repo();
    for p in ["", "../elsewhere", "/tmp"] {
        let (code, v) = r.json(&["new", "sot", "--governs", p, "--title", "X", "--write"]);
        assert_eq!(code, 2, "{p}: {v}");
        let (code, v) = r.json(&["context", p]);
        assert_eq!(code, 2, "{p}: {v}");
        assert!(!v.to_string().contains("spec new --governs"), "{v}");
    }
}

#[test]
fn c145_c146_fix_next_keeps_the_pick_and_refuses_a_foreign_one() {
    let r = common::repo();
    let a = r.new_doc("adr", "services/shop", "First readme");
    let b = r.new_doc("adr", "services/shop", "Second readme");
    for id in [&a, &b] {
        assert_eq!(r.json(&["set", id, "aliases+=README", "--write"]).0, 0);
    }
    let target = format!("{}:1", common::doc_path(&r, &a));
    let (code, v) = r.json(&["fix", "--rule", "I2", "--pick", &a, &target]);
    assert_eq!(code, 0, "{v}");
    let next = v["next"].as_str().unwrap_or_default().to_string();
    assert!(next.contains(&format!("--pick {a}")) && next.contains(&target), "{v}");
    let (code, v) = r.json(&["fix", "--rule", "I2", "--pick", "adr:root/nope", "--write", &target]);
    assert_eq!(code, 2, "{v}");
    assert!(v.to_string().contains(&b), "{v}");
}

#[test]
fn c133_new_usage_next_skips_option_values() {
    let r = common::repo();
    let (code, v) = r.json(&["new", "sot", "--governs", "services/shop", "--write"]);
    assert_eq!(code, 2, "{v}");
    let next = v["next"].as_str().unwrap_or_default();
    assert!(next.starts_with("spec new sot --title ") && next.contains("--governs services/shop"), "{v}");
    let (_, v) = r.json(&["new", "--title", "My doc", "--write"]);
    assert_eq!(v["next"], "spec new adr --title 'My doc' --write", "{v}");
}

#[test]
fn c157_edit_preview_on_crlf_matches_lf() {
    let r = common::repo();
    let lf = r.new_doc("sot", "services/shop", "Cart");
    let cr = r.new_doc("sot", "services/blog", "Post");
    let p = common::doc_path(&r, &cr);
    // An edit refreshes `updated`, so set it in the past on both: then the line changes in both
    // previews whatever second the edit runs in.
    for id in [&lf, &cr] {
        let p = common::doc_path(&r, id);
        let old: String = r
            .read(&p)
            .lines()
            .map(|l| {
                if l.starts_with("updated:") { "updated: 2000-01-01T00:00:00Z\n".to_string() } else { format!("{l}\n") }
            })
            .collect();
        r.write(&p, &old);
    }
    r.write(&p, &r.read(&p).replace('\n', "\r\n"));
    r.commit("crlf");
    let changed = |id: &str| {
        let (_, v) = r.json(&["edit", id, "--section", "Rule", "--text", "New rule."]);
        let it = v["items"].as_array().unwrap().iter().find(|i| i["op"] == "update").cloned().unwrap();
        let n = |h: &Value, k: &str| h[k].as_array().unwrap().len();
        it["hunks"].as_array().unwrap().iter().map(|h| (n(h, "before"), n(h, "after"))).collect::<Vec<_>>()
    };
    assert_eq!(changed(&cr), changed(&lf));
}

#[test]
fn edit_preview_splits_apart_changes_into_hunks() {
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart");
    let p = common::doc_path(&r, &id);
    let old: String =
        r.read(&p)
            .lines()
            .map(|l| {
                if l.starts_with("updated:") { "updated: 2000-01-01T00:00:00Z\n".to_string() } else { format!("{l}\n") }
            })
            .collect();
    r.write(&p, &old);
    r.commit("doc");
    let (code, v) = r.json(&["edit", &id, "--section", "Rule", "--text", "New rule."]);
    assert_eq!(code, 0, "{v}");
    let it = v["items"].as_array().unwrap().iter().find(|i| i["op"] == "update").cloned().unwrap();
    let hs = it["hunks"].as_array().unwrap();
    assert_eq!(hs.len(), 2, "{it}");
    assert_eq!(hs[0]["before"], json!(["updated: 2000-01-01T00:00:00Z"]), "{it}");
    assert_eq!(hs[0]["after"].as_array().unwrap().len(), 1, "{it}");
    assert!(hs[1]["after"].as_array().unwrap().iter().any(|l| l == "New rule."), "{it}");
    assert_eq!(it["line"], hs[0]["line"], "{it}");
    let (_, t) = r.text(&["edit", &id, "--section", "Rule", "--text", "New rule."]);
    assert_eq!(t.matches("@@ -").count(), 2, "{t}");
}

/// C159: preview hunk headers are the ones `git diff -U0` prints for the same write.
#[test]
fn c159_preview_hunk_headers_match_git() {
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart");
    let p = common::doc_path(&r, &id);
    let old: String =
        r.read(&p)
            .lines()
            .map(|l| {
                if l.starts_with("updated:") { "updated: 2000-01-01T00:00:00Z\n".to_string() } else { format!("{l}\n") }
            })
            .collect();
    r.write(&p, &old);
    r.commit("doc");
    let headers =
        |t: &str| t.lines().map(str::trim).filter(|l| l.starts_with("@@ ")).map(|l| l.to_string()).collect::<Vec<_>>();
    let git_headers = || {
        let out = std::process::Command::new("git").args(["diff", "-U0"]).current_dir(r.path()).output().unwrap();
        let t = String::from_utf8_lossy(&out.stdout).into_owned();
        // git adds the section heading after the second @@; spec does not.
        headers(&t)
            .into_iter()
            .map(|h| h.splitn(4, ' ').take(3).collect::<Vec<_>>().join(" ") + " @@")
            .collect::<Vec<_>>()
    };
    // An insertion (and the refreshed `updated:` line), then a deletion.
    for (text, step) in [("one\ntwo\nthree", "insert"), ("one", "delete")] {
        // An old `updated:` before each step: had the last write stamped the same second as this
        // preview, the preview would show no `updated:` hunk while the write a second later did.
        let stale: String = r
            .read(&p)
            .lines()
            .map(|l| {
                if l.starts_with("updated:") { "updated: 2000-01-01T00:00:00Z\n".to_string() } else { format!("{l}\n") }
            })
            .collect();
        r.write(&p, &stale);
        r.git(&["add", "-A"]);
        r.git(&["commit", "-qm", "stale", "--allow-empty"]);
        let (code, t) = r.text(&["edit", &id, "--section", "Rule", "--text", text]);
        assert_eq!(code, 0, "{t}");
        let shown = headers(&t);
        assert!(!shown.is_empty(), "{t}");
        let (code, w) = r.text(&["edit", &id, "--section", "Rule", "--text", text, "--write"]);
        assert_eq!(code, 0, "{w}");
        // The write refreshes `updated:` to another second than the preview may have: same header.
        assert_eq!(shown, git_headers(), "{step}: {t}");
        r.commit(step);
    }
}

/// C158: a preview of a very large change keeps memory and time small.
#[test]
fn c158_large_edit_preview_is_bounded() {
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart");
    let old: String = (1..=5000).map(|i| format!("old line {i}\n")).collect();
    r.write("old.md", &old);
    assert_eq!(r.json(&["edit", &id, "--section", "Rule", "--file", "old.md", "--write"]).0, 0);
    r.commit("big");
    let new: String = (1..=5000).map(|i| format!("new line {i}\n")).collect();
    r.write("new.md", &new);
    let t = std::time::Instant::now();
    let (code, v) = r.json(&["edit", &id, "--section", "Rule", "--file", "new.md"]);
    assert_eq!(code, 0, "{v}");
    assert!(t.elapsed() < std::time::Duration::from_secs(10), "{:?}", t.elapsed());
    let it = v["items"].as_array().unwrap().iter().find(|i| i["op"] == "update").cloned().unwrap();
    let after: usize = it["hunks"].as_array().unwrap().iter().map(|h| h["after"].as_array().unwrap().len()).sum();
    assert!(after >= 5000, "{after}");
}

/// C160: a title with a line break is refused by new and set (exit 2, nothing written).
#[test]
fn c160_titles_with_line_breaks_are_refused() {
    let r = common::repo();
    for t in ["a\nb", "a\rb"] {
        let (code, v) = r.json(&["new", "sot", "--governs", "services/shop", "--title", t, "--write"]);
        assert_eq!(code, 2, "{v}");
        assert!(v["error"].as_str().unwrap().contains("line break"), "{v}");
        assert!(v["next"].as_str().unwrap().starts_with("spec new sot --title"), "{v}");
    }
    let id = r.new_doc("sot", "services/shop", "Cart");
    r.commit("doc");
    let (code, v) = r.json(&["set", &id, "title=a\nb", "--write"]);
    assert_eq!(code, 2, "{v}");
    assert!(v["next"].as_str().unwrap().starts_with(&format!("spec set {id} title=")), "{v}");
    let (_, v) = r.json(&["show", &id]);
    assert!(v.to_string().contains("Cart"), "{v}");
    let st = std::process::Command::new("git").args(["status", "--porcelain"]).current_dir(r.path()).output().unwrap();
    assert!(st.stdout.is_empty(), "{st:?}");
}

#[test]
fn c162_unicode_line_breaks_in_titles_are_refused() {
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart");
    r.commit("doc");
    for t in ["a\u{85}b", "a\u{2028}b", "a\u{2029}b"] {
        let (code, v) = r.json(&["new", "sot", "--governs", "services/shop", "--title", t, "--write"]);
        assert_eq!(code, 2, "{v}");
        assert!(v["next"].as_str().unwrap().starts_with("spec new sot --title"), "{v}");
        let (code, v) = r.json(&["set", &id, &format!("title={t}"), "--write"]);
        assert_eq!(code, 2, "{v}");
        assert!(v["next"].as_str().unwrap().starts_with(&format!("spec set {id} title=")), "{v}");
    }
    let st = std::process::Command::new("git").args(["status", "--porcelain"]).current_dir(r.path()).output().unwrap();
    assert!(st.stdout.is_empty(), "{st:?}");
}

#[test]
fn c164_mv_to_the_same_place_is_no_change() {
    let r = common::repo();
    let d = r.new_doc("sot", "services/shop", "Cart");
    r.commit("doc");
    let slug = d.rsplit('/').next().unwrap().to_string();
    let (code, v) = r.json(&["mv", &d, "--slug", &slug, "--write"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["written"], false, "{v}");
    assert!(v.get("undo").is_none(), "{v}");
    let (_, t) = r.text(&["mv", &d, "--slug", &slug, "--write"]);
    assert!(t.contains("no change") && !t.contains("undo:"), "{t}");
}
