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
    let (code, v) = r.json(&["set", &id, "status=superseded"]);
    assert_eq!(code, 2, "{v}");
    let e = v.to_string();
    assert!(e.contains(&format!("spec new adr --title '…' --from {id}")), "{e}");
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
        let mut args = vec!["new", "sot"];
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
    assert!(v["undo"].as_str().unwrap().starts_with("git stash push"), "a clean file keeps the stash: {v}");
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
    assert!(undo.starts_with(&format!("git -C {name} ")), "{undo}");
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
    assert!(v["undo"].as_str().unwrap().starts_with("git stash push"), "{v}");
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
