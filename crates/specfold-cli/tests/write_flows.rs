//! Changing documents: `spec new`, `set`, `edit`, `mv` and `fix`.

mod common;

#[test]
fn supersede_flow_keeps_both_sides_in_step() {
    let r = common::repo();
    let old = r.new_doc("adr", "services/shop", "Use files");
    assert_eq!(r.json(&["set", &old, "status=active"]).0, 0);
    let (code, v) = r.json(&["new", "adr", "--from", &old, "--governs", "services/shop", "--title", "Use a database"]);
    assert_eq!(code, 0, "{v}");
    let new = v["id"].as_str().unwrap().to_string();
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
    let (code, _) = r.json(&["set", &id, "status=rejected"]);
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
        let (code, v) = r.json(&["set", id, "aliases+=README"]);
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
    let (code, v) = r.json(&["new", "change", "--governs", "services/shop/src", "--title", "Cart keeps coupons"]);
    assert_eq!(code, 0, "{v}");
    let id = v["id"].as_str().unwrap().to_string();
    assert!(id.starts_with("change:"), "{v}");
    assert_eq!(r.json(&["set", &id, "kind=fixed"]).0, 0);
    let (_, v) = r.json(&["find", "--type", "change", "--status", "active"]);
    assert!(v.to_string().contains(&id), "pending notes are the active ones: {v}");
    assert_eq!(r.json(&["set", &id, "status=archived", "release=0.3.0"]).0, 0);
    let (_, v) = r.json(&["find", "--type", "change", "--status", "active"]);
    assert!(!v.to_string().contains(&id), "{v}");
    let (code, v) = r.json(&["check"]);
    assert_eq!(code, 0, "kind and release are known keys: {v}");
}

#[test]
fn superseding_a_draft_points_at_activating_it_not_at_the_same_command() {
    let r = common::repo();
    let old = r.new_doc("adr", "services/shop/src", "Cart rounding");
    let (code, text) =
        r.text(&["new", "adr", "--governs", "services/shop/src", "--title", "New rounding", "--from", &old]);
    assert_ne!(code, 0, "{text}");
    assert!(text.contains(&format!("spec set {old} status=active")), "{text}");
    assert!(!text.contains("supersede it with"), "{text}");
    let path = r.json(&["resolve", &old]).1["path"].as_str().unwrap().to_string();
    r.write(&path, &r.read(&path).replace("status: draft", "status: accepted"));
    let (_, text) = r.text(&["check"]);
    assert!(text.contains("for 'accepted' use 'active'"), "{text}");
}

#[test]
fn new_refuses_a_forbidden_path_and_suggests_local() {
    let r = common::repo();
    r.write("spec.toml", &(r.read("spec.toml").replacen("[forbid]\n", "[forbid]\npaths = [\"**/.spec\"]\n", 1)));
    if !r.read("spec.toml").contains("**/.spec") {
        r.write("spec.toml", &(r.read("spec.toml") + "[forbid]\npaths = [\"**/.spec\"]\n"));
    }
    let (code, v) = r.json(&["new", "adr", "--governs", "services/shop", "--title", "Use files"]);
    assert_ne!(code, 0, "{v}");
    let e = v["error"].as_str().unwrap();
    assert!(e.contains("[forbid]"), "{e}");
    let next = v["next"].as_str().unwrap();
    assert!(next.starts_with("spec new adr ") && next.ends_with(" --local"), "{next}");
    assert!(!r.path().join("services/shop/.spec").exists(), "nothing written");
    let (code, v) = r.json(&["new", "adr", "--governs", "services/shop", "--title", "Use files", "--local"]);
    assert_eq!(code, 0, "{v}");
}

#[test]
fn set_rejecting_a_status_names_the_allowed_ones_and_the_next_command() {
    let r = common::repo();
    let id = r.new_doc("plan", "services/shop", "Checkout");
    assert_eq!(r.json(&["set", &id, "status=active"]).0, 0);
    let (code, v) = r.json(&["set", &id, "status=done"]);
    assert_ne!(code, 0);
    let e = v["error"].as_str().unwrap();
    assert!(e.contains("allowed:"), "{e}");
    assert_eq!(v["next"], format!("spec set {id} status=archived"));
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
    let (code, v) = r.json(&["set", &id, "status=done"]);
    assert_ne!(code, 0);
    let e = v["error"].as_str().unwrap();
    assert!(e.contains("allowed:") && !e.contains("next:"), "{e}");
    assert!(!v["next"].as_str().unwrap_or_default().contains("status="), "{v}");
}

#[test]
fn superseding_without_a_successor_points_to_new_from() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Use files");
    assert_eq!(r.json(&["set", &id, "status=active"]).0, 0);
    let (code, v) = r.json(&["set", &id, "status=superseded"]);
    assert_ne!(code, 0);
    assert!(v["next"].as_str().unwrap().starts_with(&format!("spec new adr --from {id} --title ")), "{v}");
    let (code, v) = r.json(&["new", "adr", "--from", &id, "--title", "Use a database"]);
    assert_eq!(code, 0, "{v}");
}

#[test]
fn set_warns_about_an_unknown_key_at_once() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Use files");
    let (code, v) = r.json(&["set", &id, "colour=blue"]);
    assert_eq!(code, 0, "{v}");
    assert!(v["warnings"][0].as_str().unwrap().contains("unknown key 'colour'"), "{v}");
    assert!(v["next"].as_str().unwrap().contains("--rule S1"), "{v}");
}

#[test]
fn a_mostly_non_ascii_title_takes_its_slug_from_governs_or_asks_for_one() {
    let r = common::repo();
    let (code, v) = r.json(&["new", "adr", "--governs", "services/shop/src/cart", "--title", "결제는 PG 하나만 쓴다"]);
    assert_eq!(code, 0, "{v}");
    assert!(v["path"].as_str().unwrap().ends_with("-cart.adr.md"), "{v}");
    assert!(v["slug_note"].as_str().unwrap().contains("--slug"), "{v}");
    let (code, v) = r.json(&["new", "adr", "--title", "결제"]);
    assert_ne!(code, 0);
    assert!(v["error"].as_str().unwrap().contains("--slug"), "{v}");
}

#[test]
fn the_first_local_document_keeps_the_local_directory_out_of_git() {
    let r = common::repo();
    let (code, v) = r.json(&["new", "plan", "--title", "Try it", "--local"]);
    assert_eq!(code, 0, "{v}");
    let status =
        std::process::Command::new("git").arg("-C").arg(r.path()).args(["status", "--porcelain"]).output().unwrap();
    assert!(!String::from_utf8_lossy(&status.stdout).contains(".spec.local"), "{v}");
    let (_, v) = r.json(&["new", "plan", "--title", "Again", "--local"]);
    assert!(v.get("ignored").is_none(), "only once: {v}");
}

#[test]
fn new_takes_the_title_as_a_positional_argument() {
    let r = common::repo();
    let (code, v) = r.json(&["new", "adr", "Use files", "--governs", "services/shop"]);
    assert_eq!(code, 0, "{v}");
    assert!(v["path"].as_str().unwrap().ends_with("-use-files.adr.md"), "{v}");
    let (code, v) = r.json(&["new", "adr"]);
    assert_ne!(code, 0);
    assert!(v["next"].as_str().unwrap().starts_with("spec new adr --title '"), "{v}");
}

#[test]
fn a_mostly_non_ascii_title_points_at_slug() {
    let r = common::repo();
    let (code, v) = r.json(&["new", "adr", "--title", "결제는 PG 하나만 쓴다", "--governs", "services/shop"]);
    assert_eq!(code, 0, "{v}");
    assert!(v["slug_note"].as_str().is_some(), "{v}");
    assert!(v["next"].as_str().unwrap().contains("--slug"), "{v}");
}

#[test]
fn mv_tier_promotes_a_local_document() {
    let r = common::repo();
    let (code, v) = r.json(&["new", "sot", "Notes", "--local"]);
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
    let (code, v) = r.json(&["new", "sot", "--title", "장바구니"]);
    assert_eq!(code, 2, "{v}");
    let next = v["next"].as_str().unwrap();
    assert!(next.starts_with("spec new sot --title") && next.ends_with("--slug my-slug"), "{v}");
}

#[test]
fn required_sections_are_in_new_documents_and_a_missing_one_names_the_command() {
    let r = common::repo();
    let toml = std::fs::read_to_string(r.path().join("spec.toml")).unwrap_or_default();
    r.write("spec.toml", &format!("{toml}\n[types.adr]\nsections = [\"Decision\", \"Risks\"]\n"));
    assert_eq!(r.text(&["new", "adr", "Pick a queue"]).0, 0);
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
