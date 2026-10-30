//! End-to-end flows: the things people and agents do every day.

mod common;

#[test]
fn placement_follows_governs_and_public_paths() {
    let r = common::repo();
    let id = r.new_doc("adr", "libs/open/src", "Open format");
    assert!(id.starts_with("adr:lib.auth/"), "public lib documents go to its companion: {id}");
    let id = r.new_doc("sot", "services/shop/src", "Shop rules");
    assert_eq!(id, "sot:shop/shop-rules");
    let id = r.new_doc("plan", "services/shop", "Cross cutting");
    assert!(id.starts_with("plan:shop/"));
    let (code, v) =
        r.json(&["new", "plan", "--governs", "services/shop", "--governs", "services/blog", "--title", "Both apps"]);
    assert_eq!(code, 0);
    assert!(v["id"].as_str().unwrap().starts_with("plan:ws/"), "{v}");
}

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
fn check_fix_loop_for_mechanical_problems() {
    let r = common::repo();
    let sot = r.new_doc("sot", "services/shop", "Cart");
    let path = r.json(&["resolve", &sot]).1["path"].as_str().unwrap().to_string();
    let text = r.read(&path).replace(&format!("id: {sot}"), "id: sot:shop/wrong");
    r.write(&path, &text);
    let (code, v) = r.json(&["check", "--rule", "S2"]);
    assert_eq!(code, 1, "{v}");
    assert_eq!(r.json(&["fix", "--rule", "S2", "--write"]).0, 0);
    assert_eq!(r.json(&["check", "--rule", "S2"]).0, 0);
}

#[test]
fn staged_check_catches_backward_status_and_sealed_edits() {
    let r = common::repo();
    let adr = r.new_doc("adr", "services/shop", "Choose");
    assert_eq!(r.json(&["set", &adr, "status=active"]).0, 0);
    let (_, v) = r.json(&["audit", "--record", "--title", "First audit", "--write"]);
    let audit_path = v["path"].as_str().unwrap().to_string();
    r.commit("decided");
    let adr_path = r.json(&["resolve", &adr]).1["path"].as_str().unwrap().to_string();
    r.write(&adr_path, &r.read(&adr_path).replace("status: active", "status: draft"));
    r.write(&audit_path, &(r.read(&audit_path) + "\nedited later\n"));
    r.git(&["add", "-A"]);
    let (code, v) = r.json(&["check", "--staged"]);
    assert_eq!(code, 1);
    let rules: Vec<&str> = v["items"].as_array().unwrap().iter().map(|f| f["rule"].as_str().unwrap()).collect();
    assert!(rules.contains(&"T2") && rules.contains(&"T3"), "{rules:?}");
}

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
fn guard_masks_terms_and_reads_a_private_list() {
    let r = common::repo();
    let private = r.path().join("../private-terms.txt");
    std::fs::write(&private, "acme-internal\n").unwrap();
    let toml = r.read("spec.toml")
        + &format!(
            "[[guards]]\nname = \"public\"\npaths = [\"libs/open/**\"]\nrange = \"all\"\ndeny_terms_file = \"{}\"\n",
            private.display()
        );
    r.write("spec.toml", &toml);
    r.write("libs/open/readme.md", "Built for ACME-Internal.\n");
    let (code, v) = r.json(&["guard"]);
    assert_eq!(code, 1, "{v}");
    let msg = v["items"][0]["message"].as_str().unwrap();
    assert!(!msg.to_lowercase().contains("acme-internal"), "{msg}");
}

#[test]
fn attach_registers_and_check_catches_changes() {
    let r = common::repo();
    let d = r.new_doc("design", "services/shop", "Cart boards");
    r.write("board.svg", "<svg/>");
    let (code, v) = r.json(&["attach", &d, "board.svg", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(r.json(&["check", "--rule", "P2", "--rule", "E1"]).0, 0);
    let base = r.json(&["files", &d]).1["base"].as_str().unwrap().to_string();
    r.write(&format!("{base}/board.svg"), "<svg>changed</svg>");
    assert_eq!(r.json(&["check", "--rule", "E1"]).0, 1);
    let inside = format!("{base}/board.svg");
    let (code, v) = r.json(&["attach", &d, &inside, "--write"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(r.read(&inside), "<svg>changed</svg>");
    assert_eq!(r.json(&["check", "--rule", "E1"]).0, 0);
}

#[test]
fn mcp_serves_the_same_commands() {
    let r = common::repo();
    r.new_doc("sot", "services/shop", "Cart");
    let input = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"spec_find\",\"arguments\":{\"words\":[\"cart\"]}}}\n";
    let out = r.spec().arg("mcp").write_stdin(input).output().unwrap();
    let lines: Vec<serde_json::Value> =
        String::from_utf8_lossy(&out.stdout).lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(lines[0]["result"]["serverInfo"]["name"], "specfold");
    let payload: serde_json::Value =
        serde_json::from_str(lines[1]["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(payload["total"], 1);
}

#[test]
fn show_counts_task_items_with_file_lines() {
    let r = common::repo();
    let id = r.new_doc("plan", "services/shop", "Checkout");
    let (_, v) = r.json(&["show", &id]);
    assert_eq!(v["tasks"]["total"], 0);
    let path = v["path"].as_str().unwrap().to_string();
    let text = r.read(&path) + "\n- [x] cart\n- [ ] payment\n```\n- [ ] sample\n```\n";
    r.write(&path, &text);
    let (code, v) = r.json(&["show", &id, "--tasks"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["tasks"], serde_json::json!({"done": 1, "open": 1, "total": 2}));
    let open = &v["task_items"][1];
    assert_eq!(open["text"], "payment");
    let line = open["line"].as_u64().unwrap() as usize;
    assert_eq!(text.lines().nth(line - 1), Some("- [ ] payment"));
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
fn stale_path_cites_get_a_fix_and_mv_rewrites_them() {
    let r = common::repo();
    let sot = r.new_doc("sot", "services/shop", "Cart rules");
    let adr = r.new_doc("adr", "services/shop", "Cart totals");
    let (_, v) = r.json(&["show", &sot]);
    let sot_path = v["path"].as_str().unwrap().to_string();
    let (_, v) = r.json(&["show", &adr]);
    let adr_path = v["path"].as_str().unwrap().to_string();
    let text = r.read(&adr_path) + &format!("\nSee {sot_path} and old/.spec/sots/cart-rules.md.\n");
    r.write(&adr_path, &text);
    let (_, v) = r.json(&["check", "--rule", "L2"]);
    let fixes = v["items"].as_array().unwrap().iter().filter(|f| !f["fix"].is_null()).count();
    assert_eq!(fixes, 2, "{v}");
    let (code, v) = r.json(&["mv", &sot, "--slug", "basket-rules", "--write"]);
    assert_eq!(code, 0, "{v}");
    let new_id = v["to"].as_str().unwrap().to_string();
    let after = r.read(&adr_path);
    assert!(after.contains(&format!("See [[{new_id}]] and")), "{after}");
}

#[test]
fn check_new_lists_documents_not_in_the_last_commit() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Fresh idea");
    let (code, v) = r.json(&["check", "--new"]);
    assert_eq!(code, 0, "{v}");
    let ids: Vec<&str> = v["new_docs"].as_array().unwrap().iter().map(|d| d["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&id.as_str()), "{v}");
}

#[test]
fn check_summary_counts_per_rule_and_quiet_hides_warnings() {
    let r = common::repo();
    let adr = r.new_doc("adr", "services/shop", "Totals");
    let (_, v) = r.json(&["show", &adr]);
    let path = v["path"].as_str().unwrap().to_string();
    r.write(&path, &(r.read(&path) + "\n[[nothing-here]]\n"));
    let (_, v) = r.json(&["check", "--summary"]);
    assert_eq!(v["by_rule"]["L1"]["errors"], 1, "{v}");
    assert!(v["items"].as_array().unwrap().iter().all(|i| i.get("rule").is_some() && i.get("errors").is_some()), "{v}");
    let (_, v) = r.json(&["check", "-q"]);
    assert!(v["items"].as_array().unwrap().iter().all(|i| i["severity"] == "error"), "{v}");
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
fn audit_reports_a_plan_whose_tasks_and_status_disagree() {
    let r = common::repo();
    let id = r.new_doc("plan", "services/shop", "Checkout v2");
    assert_eq!(r.json(&["set", &id, "status=active"]).0, 0, "plan can go active");
    let (code, _) = r.json(&["edit", &id, "--text=## Steps\n\n- [x] build\n- [x] ship\n", "--write"]);
    assert_eq!(code, 0);
    let (_, v) = r.json(&["audit", "--kind", "finished"]);
    assert!(v.to_string().contains("all 2 tasks are done but the plan is still active"), "{v}");
    assert_eq!(r.json(&["edit", &id, "--text=## Steps\n\n- [x] build\n- [ ] ship\n", "--write"]).0, 0);
    let (_, v) = r.json(&["audit", "--kind", "finished"]);
    assert!(!v.to_string().contains("tasks are done"), "{v}");
    assert_eq!(r.json(&["set", &id, "status=archived"]).0, 0);
    let (_, v) = r.json(&["audit", "--kind", "finished"]);
    assert!(v.to_string().contains("archived with 1 of 2 tasks still open"), "{v}");
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
fn assets_lists_declared_skills_rules_and_instructions() {
    let r = common::repo();
    let (_, v) = r.json(&["assets"]);
    assert_eq!(v["total"], 0, "nothing declared, nothing read: {v}");
    let toml = r.read("spec.toml")
        + "[[assets]]\nkind = \"skill\"\npath = \".agents/skills\"\n[[assets]]\nkind = \"rule\"\npath = \".cursor/rules/*.mdc\"\n[[assets]]\nkind = \"instructions\"\npath = \"AGENTS.md\"\n";
    r.write("spec.toml", &toml);
    r.write(".agents/skills/pdf-tools/SKILL.md", "---\nname: pdf-tools\ndescription: Read PDFs.\n---\n# PDF\n");
    r.write(".agents/skills/pdf-tools/notes.md", "not a skill file\n");
    r.write(".cursor/rules/style.mdc", "---\ndescription: Style\nalwaysApply: true\n---\nUse tabs.\n");
    r.write(".cursor/rules/readme.txt", "ignored\n");
    r.write("AGENTS.md", "# Agents\n");
    let (code, v) = r.json(&["assets"]);
    assert_eq!(code, 0, "{v}");
    let paths: Vec<&str> = v["items"].as_array().unwrap().iter().map(|i| i["path"].as_str().unwrap()).collect();
    assert_eq!(paths, [".agents/skills/pdf-tools/SKILL.md", ".cursor/rules/style.mdc", "AGENTS.md"], "{v}");
    assert_eq!(v["items"][0]["name"], "pdf-tools");
    assert_eq!(v["items"][0]["description"], "Read PDFs.");
    let (_, v) = r.json(&["assets", "--kind", "rule"]);
    assert_eq!(v["total"], 1, "{v}");
    assert_eq!(v["items"][0]["name"], "style");
}

#[test]
fn audit_names_skill_copies_that_drift_from_agents_and_claude_without_agents() {
    let r = common::repo();
    let toml = r.read("spec.toml")
        + "[[assets]]\nkind = \"skill\"\npath = \".agents/skills\"\n[[assets]]\nkind = \"skill\"\npath = \".claude/skills\"\n";
    r.write("spec.toml", &toml);
    let skill = "---\nname: pdf-tools\ndescription: Read PDFs.\n---\n# PDF\n";
    r.write(".agents/skills/pdf-tools/SKILL.md", skill);
    r.write(".claude/skills/pdf-tools/SKILL.md", skill);
    r.write("AGENTS.md", "# Agents\n");
    r.write("CLAUDE.md", "@AGENTS.md\n");
    let (_, v) = r.json(&["audit", "--kind", "drift"]);
    assert_eq!(v["items"].as_array().map_or(0, |a| a.len()), 0, "an identical copy and an include are quiet: {v}");
    r.write(".claude/skills/pdf-tools/SKILL.md", &skill.replace("# PDF", "# PDF, edited"));
    r.write(".claude/skills/orphan/SKILL.md", "---\nname: orphan\ndescription: O.\n---\n");
    r.write("CLAUDE.md", "# Claude\n");
    let (_, v) = r.json(&["audit", "--kind", "drift"]);
    let text = v.to_string();
    assert!(text.contains("differs from .agents/skills/pdf-tools, the canonical copy"), "{v}");
    assert!(text.contains("skill 'orphan' has no source under .agents/skills"), "{v}");
    assert!(text.contains("does not include AGENTS.md"), "{v}");
}

#[test]
fn new_skill_rule_and_agent_write_templates_without_ids() {
    let r = common::repo();
    let toml = r.read("spec.toml") + "[[assets]]\nkind = \"skill\"\npath = \".agents/skills\"\n";
    r.write("spec.toml", &toml);
    let (code, v) =
        r.json(&["new", "skill", "--title", "PDF tools", "--description", "Read and fill PDFs.", "--dry-run"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["path"], ".agents/skills/pdf-tools/SKILL.md", "{v}");
    assert!(!r.path().join(".agents/skills/pdf-tools/SKILL.md").exists(), "a dry run writes nothing");
    assert_eq!(r.json(&["new", "skill", "--title", "PDF tools", "--description", "Read and fill PDFs."]).0, 0);
    let text = r.read(".agents/skills/pdf-tools/SKILL.md");
    assert!(text.starts_with("---\nname: pdf-tools\ndescription: Read and fill PDFs.\n---\n"), "{text}");
    assert!(!text.contains("id:"), "{text}");
    let (code, _) = r.json(&["new", "skill", "--title", "PDF tools"]);
    assert_ne!(code, 0, "an existing skill is not overwritten");
    let (code, v) = r.json(&["new", "rule", "--title", "Code style"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["path"], ".cursor/rules/code-style.mdc", "{v}");
    assert_eq!(v["declared"], false, "{v}");
    assert_eq!(r.json(&["new", "agent", "--title", "Reviewer"]).1["path"], ".claude/agents/reviewer.md");
    let (_, v) = r.json(&["check", "--rule", "A1"]);
    assert_eq!(v["errors"], 0, "templates pass the asset rule: {v}");
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
fn agents_lines_name_edit_check_assets_and_audit() {
    let r = common::repo();
    let (code, v) = r.json(&["agents", "--for", "generic"]);
    assert_eq!(code, 0, "{v}");
    let text = v["lines"].to_string();
    for want in [
        "spec edit",
        "--row",
        "spec check --staged",
        "--summary",
        "spec assets",
        "spec new skill",
        "--kind drift",
        "--kind finished",
        "spec:count",
    ] {
        assert!(text.contains(want), "missing {want}: {text}");
    }
    assert!(v["lines"].as_array().unwrap().len() <= 8, "keep it short: {text}");
}

#[test]
fn assets_and_show_list_the_documents_an_asset_names() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Use a database");
    let (_, v) = r.json(&["show", &id]);
    let path = v["path"].as_str().unwrap().to_string();
    assert_eq!(v["assets"], serde_json::json!([]), "{v}");
    let toml = r.read("spec.toml") + "[[assets]]\nkind = \"skill\"\npath = \".agents/skills\"\n";
    r.write("spec.toml", &toml);
    let rel = format!("../../../{path}");
    r.write(
        ".agents/skills/db/SKILL.md",
        &format!("---\nname: db\ndescription: Database work.\n---\nRead `{id}` and [the record]({rel}) first. See https://example.com/x.md too.\n"),
    );
    r.write(
        ".agents/skills/other/SKILL.md",
        "---\nname: other\ndescription: Nothing.\n---\nNo links here: ratio 3:4.\n",
    );
    let before = r.read(".agents/skills/db/SKILL.md");
    let (code, v) = r.json(&["assets"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["items"][0]["docs"], serde_json::json!([id]), "id and relative path name one document: {v}");
    assert_eq!(v["items"][1]["docs"], serde_json::json!([]), "{v}");
    let (_, v) = r.json(&["show", &id]);
    assert_eq!(v["assets"][0]["path"], ".agents/skills/db/SKILL.md", "{v}");
    assert_eq!(v["assets"].as_array().unwrap().len(), 1, "{v}");
    let (_, t) = r.text(&["show", &id]);
    assert!(t.contains("assets:") && t.contains("<- skill .agents/skills/db/SKILL.md"), "{t}");
    assert_eq!(r.read(".agents/skills/db/SKILL.md"), before, "assets are only read");
}

#[test]
fn a_pinned_document_is_stale_after_any_commit_to_its_code() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Cart totals");
    assert_eq!(r.json(&["set", &id, "status=active"]).0, 0);
    let (code, v) = r.json(&["set", &id, "verified_at=HEAD"]);
    assert_eq!(code, 0, "{v}");
    let (_, v) = r.json(&["show", &id]);
    let pin = v["frontmatter"]["verified_at"].as_str().unwrap_or_default().to_string();
    assert_eq!(pin.len(), 12, "HEAD is stored as a short hash: {v}");
    r.commit("pinned");
    let (code, v) = r.json(&["check"]);
    assert_eq!(code, 0, "verified_at is a known key: {v}");
    let (_, v) = r.json(&["audit", "--kind", "stale"]);
    assert!(!v.to_string().contains("since verified_at"), "nothing changed yet: {v}");
    r.write("services/shop/src/cart/total.rs", "fn total() -> u32 { 0 }\n");
    r.commit("change totals");
    let (_, v) = r.json(&["audit", "--kind", "stale"]);
    let t = v.to_string();
    assert!(t.contains("changed in 1 commit(s) since verified_at"), "{v}");
    assert!(t.contains("verified_at=HEAD"), "the next step re-pins: {v}");
    let (_, v) = r.json(&["set", &id, "verified_at=HEAD"]);
    assert_eq!(v["id"], id.as_str());
    let (_, v) = r.json(&["audit", "--kind", "stale"]);
    assert!(!v.to_string().contains("since verified_at"), "re-pinned: {v}");
    assert_eq!(r.json(&["set", &id, "verified_at=0000000deadbeef"]).0, 0);
    let (_, v) = r.json(&["audit", "--kind", "stale"]);
    assert!(v.to_string().contains("is not a commit in this repository"), "{v}");
}

#[test]
fn check_extra_keys_allow_project_keys_on_every_type() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Cart totals");
    let path = r.json(&["resolve", &id]).1["path"].as_str().unwrap().to_string();
    r.write(&path, &r.read(&path).replacen("title:", "team: payments\nreview_by: 2027-01-01\nsquad: x\ntitle:", 1));
    let (_, v) = r.json(&["check"]);
    let t = v.to_string();
    assert!(t.contains("unknown key 'team'") && t.contains("[check] extra_keys"), "{v}");
    let toml = r.read("spec.toml") + "[check]\nextra_keys = [\"team\", \"review_*\"]\n";
    r.write("spec.toml", &toml);
    let (_, v) = r.json(&["check"]);
    let t = v.to_string();
    assert!(!t.contains("unknown key 'team'") && !t.contains("unknown key 'review_by'"), "{v}");
    assert!(t.contains("unknown key 'squad'"), "only listed keys pass: {v}");
}

#[test]
fn sync_assets_previews_then_copies_only_into_declared_copy_roots() {
    let r = common::repo();
    let base = r.read("spec.toml");
    let copy = "[[assets]]\nkind = \"skill\"\npath = \".agents/skills\"\n[[assets]]\nkind = \"skill\"\npath = \".claude/skills\"\ncopy_of = \".agents/skills\"\n";
    r.write("spec.toml", &(base.clone() + copy));
    let skill = "---\nname: pdf-tools\ndescription: Read PDFs.\n---\n# PDF\n";
    r.write(".agents/skills/pdf-tools/SKILL.md", skill);
    r.write(".agents/skills/pdf-tools/scripts/run.sh", "echo pdf\n");
    r.write(".claude/skills/pdf-tools/SKILL.md", "---\nname: pdf-tools\ndescription: Old.\n---\n");
    r.write(".claude/skills/stale/SKILL.md", "---\nname: stale\ndescription: S.\n---\n");
    let (_, v) = r.json(&["audit", "--kind", "drift"]);
    assert!(v.to_string().contains("spec sync assets --write"), "drift points at sync for a copy root: {v}");

    let (code, v) = r.json(&["sync", "assets"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["written"], false, "preview is the default: {v}");
    assert_eq!(v["changes"], 2, "{v}");
    assert!(r.read(".claude/skills/pdf-tools/SKILL.md").contains("Old."), "a preview writes nothing");

    let (code, v) = r.json(&["sync", "assets", "--write"]);
    assert_eq!((code, v["written"].clone()), (0, serde_json::json!(true)), "{v}");
    assert_eq!(r.read(".claude/skills/pdf-tools/SKILL.md"), skill);
    assert_eq!(r.read(".claude/skills/pdf-tools/scripts/run.sh"), "echo pdf\n");
    assert!(r.path().join(".claude/skills/stale/SKILL.md").is_file(), "nothing is deleted without --prune");
    assert_eq!(r.read(".agents/skills/pdf-tools/SKILL.md"), skill, "the source is never written");

    let (code, v) = r.json(&["sync", "assets", "--prune", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert!(!r.path().join(".claude/skills/stale/SKILL.md").exists(), "--prune removes what the source lacks");
    let (_, v) = r.json(&["sync", "assets"]);
    assert_eq!(v["changes"], 0, "{v}");

    let (_, v) = r.json(&["fix", "--write"]);
    assert!(!v.to_string().contains(".claude/skills"), "spec fix never touches assets: {v}");
}

#[test]
fn sync_assets_refuses_paths_that_leave_the_repository_or_overlap_the_source() {
    let r = common::repo();
    let base = r.read("spec.toml");
    r.write(".agents/skills/pdf-tools/SKILL.md", "---\nname: pdf-tools\ndescription: D.\n---\n");
    let outside =
        r.path().parent().unwrap().join(format!("{}-outside", r.path().file_name().unwrap().to_string_lossy()));
    for (path, src) in [
        ("../escape", ".agents/skills"),
        ("/tmp/escape", ".agents/skills"),
        (".claude/skills", "../elsewhere"),
        (".claude/*", ".agents/skills"),
        (".agents/skills/inner", ".agents/skills"),
        (".agents", ".agents/skills"),
    ] {
        r.write("spec.toml", &format!("{base}[[assets]]\nkind = \"skill\"\npath = \"{path}\"\ncopy_of = \"{src}\"\n"));
        let (code, v) = r.json(&["sync", "assets", "--write"]);
        assert_ne!(code, 0, "{path} <- {src} must be refused: {v}");
    }
    assert!(!r.path().join(".agents/skills/inner").exists() && !r.path().parent().unwrap().join("escape").exists());

    #[cfg(unix)]
    {
        std::fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, r.path().join(".claude")).unwrap();
        r.write(
            "spec.toml",
            &format!("{base}[[assets]]\nkind = \"skill\"\npath = \".claude/skills\"\ncopy_of = \".agents/skills\"\n"),
        );
        let (code, v) = r.json(&["sync", "assets", "--write"]);
        assert_ne!(code, 0, "a copy root behind a symbolic link is refused: {v}");
        assert!(std::fs::read_dir(&outside).unwrap().next().is_none(), "nothing was written through the link");
        std::fs::remove_dir_all(&outside).unwrap();
    }
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
fn graph_draws_trees_diagrams_relations_and_chains_both_ways() {
    let r = common::repo();
    let old = r.new_doc("adr", "services/shop", "Use files");
    assert_eq!(r.json(&["set", &old, "status=active"]).0, 0);
    let (_, v) = r.json(&["new", "adr", "--from", &old, "--governs", "services/shop", "--title", "Use a database"]);
    let new = v["id"].as_str().unwrap().to_string();
    let sot = r.new_doc("sot", "services/shop/src", "Cart");
    let plan = r.new_doc("plan", "services/shop", "Ship");
    assert_eq!(r.json(&["set", &plan, &format!("rel.implements={sot}")]).0, 0);

    let (code, text) = r.text(&["graph"]);
    assert_eq!(code, 0, "{text}");
    assert!(text.contains(&format!("├─ supersedes → {old}")), "{text}");
    assert!(text.contains("governs → services/shop"), "{text}");
    let (_, text) = r.text(&["graph", "--ascii"]);
    assert!(text.contains(&format!("|-- supersedes -> {old}")) && !text.contains("├"), "{text}");
    let (_, text) = r.text(&["graph", "--depth", "1"]);
    assert!(!text.contains("    "), "depth 1 shows only direct relations: {text}");

    let (_, text) = r.text(&["graph", "--format", "mermaid"]);
    assert!(
        text.starts_with("graph LR") && text.contains("-->|supersedes|") && text.contains("-.->|governs|"),
        "{text}"
    );
    assert!(!text.contains("next:"), "a diagram is only the diagram: {text}");
    let (_, text) = r.text(&["graph", "--format", "dot"]);
    assert!(
        text.starts_with("digraph specfold {")
            && text.contains(&format!("\"{new}\" -> \"{old}\" [label=\"supersedes\"]")),
        "{text}"
    );
    assert_ne!(r.text(&["graph", "impact", &sot, "--format", "dot"]).0, 0);

    let (_, v) = r.json(&["graph", "impact", &sot]);
    assert_eq!((v["items"][0]["kind"].as_str(), v["items"][0]["depth"].as_u64()), (Some("implements"), Some(1)), "{v}");
    let (_, v) = r.json(&["graph"]);
    assert!(v["governs"].to_string().contains("services/shop/src"), "governs is in --json: {v}");

    let (_, v) = r.json(&["graph", "chain", &new]);
    let ids: Vec<_> = v["items"].as_array().unwrap().iter().map(|i| i["id"].as_str().unwrap().to_string()).collect();
    assert_eq!(ids, vec![old.clone(), new.clone()], "the chain from the new document shows what it replaced");
    assert_eq!(v["items"][1]["this"], true);
}

#[test]
fn box_graph_fits_columns_and_falls_back_to_ascii_or_the_tree() {
    let r = common::repo();
    let sot = r.new_doc("sot", "services/shop/src", "Cart");
    let plan = r.new_doc("plan", "services/shop", "Ship");
    assert_eq!(r.json(&["set", &plan, &format!("rel.implements={sot}")]).0, 0);
    let out = |cols: &str, locale: &str, extra: &[&str]| {
        let mut c = r.spec();
        c.args(["graph", "--format", "box"]).args(extra).env("COLUMNS", cols).env("LC_ALL", locale);
        String::from_utf8_lossy(&c.output().unwrap().stdout).into_owned()
    };
    let text = out("120", "C.UTF-8", &[]);
    assert!(text.contains("┌") && text.contains("i▶") && text.contains("i implements"), "{text}");
    let widest = text.lines().filter(|l| !l.starts_with("next:")).map(|l| l.chars().count()).max().unwrap();
    assert!(widest <= 120, "{text}");
    let text = out("120", "C", &[]);
    assert!(text.contains("i>") && text.lines().all(|l| l.is_ascii()), "a non-UTF-8 locale draws ASCII: {text}");
    assert!(out("120", "C.UTF-8", &["--ascii"]).contains("+-"));
    let text = out("30", "C.UTF-8", &[]);
    assert!(text.contains("COLUMNS is 30") && text.contains("implements →"), "too narrow shows the tree: {text}");
    let (_, v) = r.json(&["graph", "--format", "box"]);
    assert!(v["diagram"].as_str().unwrap().contains(&sot), "{v}");
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
fn next_lines_name_real_rules_and_paths_lead_to_governs() {
    let r = common::repo();
    let (_, v) = r.json(&["explain", "Z1"]);
    assert_eq!(v["next"], "spec check --rule Z1");
    let (code, v) = r.json(&["graph", "impact", "services/shop/src"]);
    assert_ne!(code, 0);
    assert_eq!(v["next"], "spec graph governs services/shop/src", "{v}");
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
fn fail_on_none_reports_and_names_what_it_left_out() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Use files");
    let (code, v) = r.json(&["show", &id]);
    assert_eq!(code, 0);
    let path = r.path().join(v["path"].as_str().unwrap());
    let doc: String = std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .map(|l| if l.starts_with("status:") { "status: bogus\n".to_string() } else { format!("{l}\n") })
        .collect();
    std::fs::write(&path, doc).unwrap();
    let (code, v) = r.json(&["check", "--fail-on", "none"]);
    assert_eq!(code, 0, "{v}");
    assert!(v["not_failing"]["S4"].as_u64().unwrap() >= 1, "{v}");
    let (code, text) = r.text(&["check", "--fail-on", "L1"]);
    assert_eq!(code, 0, "{text}");
    assert!(text.contains("S4: ") && text.contains("left out of the exit code"), "{text}");
}

#[test]
fn a_missing_target_suggests_a_real_id() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Use files");
    for args in [&["graph", "impact"][..], &["graph", "chain"][..], &["show", "--tasks"][..]] {
        let (code, v) = r.json(args);
        assert_ne!(code, 0, "{args:?}");
        let next = v["next"].as_str().unwrap_or_default();
        assert!(next.contains(&id), "{args:?}: {v}");
    }
    let (_, v) = r.json(&["graph", "tree"]);
    assert_eq!(v["next"], "spec graph", "{v}");
}

#[test]
fn find_open_tasks_lists_unfinished_documents() {
    let r = common::repo();
    let done = r.new_doc("plan", "services/shop", "Done plan");
    let open = r.new_doc("plan", "services/shop", "Open plan");
    for (id, body) in [(&done, "- [x] ship\n"), (&open, "- [ ] ship\n- [ ] test\n- [x] plan\n")] {
        let (_, v) = r.json(&["show", id]);
        let path = r.path().join(v["path"].as_str().unwrap());
        let mut text = std::fs::read_to_string(&path).unwrap();
        text.push_str(&format!("\n{body}"));
        std::fs::write(&path, text).unwrap();
    }
    let (code, v) = r.json(&["find", "--open-tasks"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["total"], 1, "{v}");
    assert_eq!(v["items"][0]["id"], open.as_str(), "{v}");
    assert_eq!(v["items"][0]["open_tasks"], 2, "{v}");
    assert_eq!(v["next"], format!("spec show {open} --tasks"), "{v}");
}

#[test]
fn graph_impact_lists_assets_that_name_the_document() {
    let r = common::repo();
    let toml = r.read("spec.toml") + "[[assets]]\nkind = \"instructions\"\npath = \"AGENTS.md\"\n";
    r.write("spec.toml", &toml);
    let id = r.new_doc("sot", "services/shop", "Cart");
    r.write("AGENTS.md", &format!("# Agents\n\nRead {id} before changing the cart.\n"));
    let (code, v) = r.json(&["graph", "impact", &id]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["assets"][0]["path"], "AGENTS.md", "{v}");
    assert_eq!(v["assets"][0]["names"][0], id.as_str(), "{v}");
    let (_, text) = r.text(&["graph", "impact", &id]);
    assert!(text.contains("asset instructions AGENTS.md  names "), "{text}");
}

#[test]
fn max_bytes_keeps_whole_items_and_points_at_the_rest() {
    let r = common::repo();
    for t in ["Cart", "Pay", "Ship"] {
        r.new_doc("sot", "services/shop", t);
    }
    let (_, all) = r.json(&["find"]);
    assert_eq!(all["items"].as_array().unwrap().len(), 3);
    assert!(all.get("cut_by").is_none());
    let (code, v) = r.json(&["find", "--max-bytes", "1"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["items"].as_array().unwrap().len(), 1, "always at least one whole item");
    assert_eq!(v["more"], 2);
    assert_eq!(v["cut_by"], "max_bytes");
    assert_eq!(v["items"][0], all["items"][0]);
    let (_, v) = r.json(&["find", "--max-bytes", "1", "--offset", "1"]);
    assert_eq!(v["items"][0], all["items"][1]);
    let input = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"spec_find\",\"arguments\":{\"max_bytes\":1}}}\n";
    let out = r.spec().arg("mcp").write_stdin(input).output().unwrap();
    let line: serde_json::Value =
        serde_json::from_str(String::from_utf8_lossy(&out.stdout).lines().next().unwrap()).unwrap();
    let payload: serde_json::Value =
        serde_json::from_str(line["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(payload["more"], 2, "{payload}");
}

#[test]
fn graph_assets_flag_adds_asset_nodes_to_the_whole_graph() {
    let r = common::repo();
    let toml = r.read("spec.toml") + "[[assets]]\nkind = \"instructions\"\npath = \"AGENTS.md\"\n";
    r.write("spec.toml", &toml);
    let id = r.new_doc("sot", "services/shop", "Cart");
    r.write("AGENTS.md", &format!("# Agents\n\nRead {id} first.\n"));
    let (_, v) = r.json(&["graph"]);
    assert!(v.get("assets").is_none(), "off by default: {v}");
    let (code, v) = r.json(&["graph", "--assets"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["assets"][0]["names"][0], id.as_str(), "{v}");
    let (_, m) = r.text(&["graph", "--assets", "--format", "mermaid"]);
    assert!(m.contains("-.->|names| d0"), "{m}");
    let (_, d) = r.text(&["graph", "--assets", "--format", "dot"]);
    assert!(d.trim_end().ends_with('}') && d.contains("[label=\"names\""), "{d}");
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
fn show_body_splits_at_headings_under_max_bytes() {
    let r = common::repo();
    let id = r.new_doc("plan", "services/shop", "Checkout");
    let (_, v) = r.json(&["show", &id]);
    let path = v["path"].as_str().unwrap().to_string();
    let long = "x".repeat(400);
    r.write(
        &path,
        &(r.read(&path) + &format!("\n## Alpha\n\n{long}\n\n## Beta\n\n```\n# not a heading\n```\n{long}\n")),
    );
    let (_, full) = r.json(&["show", &id, "--body"]);
    assert!(full.get("cut_by").is_none());
    let (code, v) = r.json(&["show", &id, "--body", "--max-bytes", "600"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["cut_by"], "max_bytes", "{v}");
    assert_eq!(v["rest_sections"], serde_json::json!(["Beta"]), "{v}");
    assert!(v["next"].as_str().unwrap().contains("--section 'Beta'"), "{v}");
    assert!(!v["body"].as_str().unwrap().contains("## Beta"));
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
fn guards_skip_spec_toml_and_explain_an_empty_scope() {
    let r = common::repo();
    let toml = r.read("spec.toml")
        + "[[guards]]\nname = \"all\"\npaths = [\"**\"]\nrange = \"all\"\ndeny_terms = [\"acme-internal\"]\n"
        + "[[guards]]\nname = \"none\"\npaths = [\"nowhere/**\"]\nrange = \"all\"\nfail_if_empty = true\ndeny_terms = [\"x\"]\n";
    r.write("spec.toml", &toml);
    r.git(&["add", "-A"]);
    let (code, v) = r.json(&["guard", "all", "--all"]);
    assert_eq!(code, 0, "spec.toml must not trip its own guard: {v}");
    let (code, text) = r.text(&["guard", "none", "--all"]);
    assert_ne!(code, 0, "{text}");
    assert!(text.contains("fail_if_empty = false"), "{text}");
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
fn check_with_only_warnings_points_at_them() {
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart");
    let (_, v) = r.json(&["show", &id]);
    let path = v["path"].as_str().unwrap().to_string();
    r.write(&path, &r.read(&path).replacen("status:", "colour: red\nstatus:", 1));
    let (code, v) = r.json(&["check"]);
    assert_eq!(code, 0, "{v}");
    assert!(v["warnings"].as_u64().unwrap() > 0, "{v}");
    let next = v["next"].as_str().unwrap();
    assert!(!next.starts_with("spec audit"), "{next}");
    assert!(next.starts_with("spec fix") || next.starts_with("spec explain S1"), "{next}");
}

#[test]
fn sync_says_when_there_are_no_agent_files() {
    let r = common::repo();
    let (_, text) = r.text(&["sync", "assets"]);
    assert!(text.contains("no agent files found"), "{text}");
    r.write("AGENTS.md", "# agents\n");
    let (_, text) = r.text(&["sync", "assets"]);
    assert!(text.contains("agent files found (AGENTS.md)"), "{text}");
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
fn graph_json_and_id_prefixes_get_real_next_steps() {
    let r = common::repo();
    let id = r.new_doc("plan", "services/shop", "Checkout");
    let (_, text) = r.text(&["graph", "json"]);
    assert!(text.contains("spec graph --json"), "{text}");
    let prefix = &id[..id.len() - 4];
    let (code, text) = r.text(&["show", prefix]);
    assert_ne!(code, 0);
    assert!(text.contains(&id), "{text}");
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
fn check_reports_only_the_named_file() {
    let r = common::repo();
    let a = r.new_doc("sot", "services/shop", "Cart");
    let b = r.new_doc("sot", "services/shop", "Till");
    let path = |id: &str| r.json(&["show", id]).1["path"].as_str().unwrap().to_string();
    let (pa, pb) = (path(&a), path(&b));
    for p in [&pa, &pb] {
        let t = r.read(p);
        r.write(p, &format!("{t}\nsee [[sot:services/nowhere]]\n"));
    }
    let (_, all) = r.json(&["check"]);
    let (code, v) = r.json(&["check", &pa]);
    assert_ne!(code, 0, "{v}");
    let items = v["items"].as_array().unwrap();
    assert!(!items.is_empty(), "{v}");
    assert!(items.iter().all(|i| i["path"] == pa.as_str()), "{v}");
    assert!(items.len() < all["items"].as_array().unwrap().len(), "{all}");
    let (_, text) = r.text(&["check", &pb, "--format", "editor"]);
    let line = text.lines().next().unwrap();
    assert!(line.starts_with(&format!("{pb}:")), "{text}");
    assert!(line.contains(": error ") || line.contains(": warning "), "{text}");
    assert!(text.lines().all(|l| l.starts_with(&format!("{pb}:"))), "{text}");
}

#[test]
fn check_reads_an_unsaved_buffer_from_stdin() {
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart");
    let path = r.json(&["show", &id]).1["path"].as_str().unwrap().to_string();
    let saved = r.read(&path);
    let (_, clean) = r.text(&["check", &path, "--format", "editor"]);
    let buffer = format!("{saved}\nsee [[sot:services/nowhere]]\n");
    let out =
        r.spec().args(["check", "--stdin-path", &path, "--format", "editor"]).write_stdin(buffer).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(text.lines().count() > clean.lines().count(), "{clean}\n---\n{text}");
    assert!(text.lines().all(|l| l.starts_with(&format!("{path}:"))), "{text}");
    assert_eq!(r.read(&path), saved, "the file on disk is left alone");
    let (_, again) = r.text(&["check", &path, "--format", "editor"]);
    assert_eq!(again, clean, "the buffer must not stay in the scan cache");
}

#[test]
fn check_rejects_files_that_are_not_documents() {
    let r = common::repo();
    let out = r.spec().args(["check", "nowhere.md"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stdout).contains("no such file"), "{out:?}");
    r.write("NOTES.md", "# notes\n");
    let out = r.spec().args(["check", "NOTES.md"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stdout).contains("not a document"), "{out:?}");
    let out = r.spec().args(["check", "--stdin-path", "gone.md"]).write_stdin("x").output().unwrap();
    assert_eq!(out.status.code(), Some(2), "{out:?}");
}

fn lsp_frames(msgs: &[serde_json::Value]) -> String {
    msgs.iter()
        .map(|m| {
            let b = m.to_string();
            format!("Content-Length: {}\r\n\r\n{b}", b.len())
        })
        .collect()
}

fn lsp_replies(out: &[u8]) -> Vec<serde_json::Value> {
    let text = String::from_utf8_lossy(out);
    text.split("Content-Length: ")
        .filter(|s| !s.is_empty())
        .map(|s| serde_json::from_str(s.split_once("\r\n\r\n").unwrap().1).unwrap())
        .collect()
}

#[test]
fn lsp_publishes_diagnostics_for_buffers() {
    use serde_json::json;
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart");
    let path = r.json(&["show", &id]).1["path"].as_str().unwrap().to_string();
    let abs = r.path().canonicalize().unwrap();
    let uri = format!("file://{}/{path}", abs.display());
    let dir = std::path::Path::new(&path).parent().unwrap().to_str().unwrap().to_string();
    let new_uri = format!("file://{}/{dir}/fresh.sot.md", abs.display());
    let broken = format!("{}\nsee [[sot:services/nowhere]]\n", r.read(&path));
    let input = lsp_frames(&[
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"rootUri": format!("file://{}", abs.display())}}),
        json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
        json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {"uri": uri, "languageId": "markdown", "version": 1, "text": broken}}}),
        json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {"uri": new_uri, "languageId": "markdown", "version": 1, "text": "# Fresh\nsee [[sot:services/nowhere]]\n"}}}),
        json!({"jsonrpc": "2.0", "method": "textDocument/didClose", "params": {"textDocument": {"uri": uri}}}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "nope", "params": {}}),
        json!({"jsonrpc": "2.0", "id": 3, "method": "shutdown"}),
        json!({"jsonrpc": "2.0", "method": "exit"}),
    ]);
    let out = r.spec().arg("lsp").write_stdin(input).output().unwrap();
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let replies = lsp_replies(&out.stdout);
    assert_eq!(replies[0]["result"]["capabilities"]["textDocumentSync"]["change"], 1, "{replies:?}");
    let diags: Vec<&serde_json::Value> =
        replies.iter().filter(|m| m["method"] == "textDocument/publishDiagnostics").collect();
    assert_eq!(diags.len(), 3, "{replies:?}");
    assert_eq!(diags[0]["params"]["uri"], uri.as_str());
    let first = diags[0]["params"]["diagnostics"].as_array().unwrap();
    assert!(first.iter().any(|d| d["source"] == "spec" && d["severity"] == 1), "{replies:?}");
    assert!(!diags[1]["params"]["diagnostics"].as_array().unwrap().is_empty(), "unsaved new file: {replies:?}");
    assert!(diags[2]["params"]["diagnostics"].as_array().unwrap().is_empty(), "close clears");
    assert!(replies.iter().any(|m| m["id"] == 2 && m["error"]["code"] == -32601), "{replies:?}");
    assert!(replies.iter().any(|m| m["id"] == 3 && m["result"].is_null()), "{replies:?}");
    assert!(!r.path().join(&dir).join("fresh.sot.md").exists());
}

#[test]
fn check_stdin_path_accepts_a_new_file_only_in_a_spec_directory() {
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart");
    let path = r.json(&["show", &id]).1["path"].as_str().unwrap().to_string();
    let dir = std::path::Path::new(&path).parent().unwrap().to_str().unwrap().to_string();
    let fresh = format!("{dir}/fresh.sot.md");
    let out = r
        .spec()
        .args(["check", "--stdin-path", &fresh, "--format", "editor"])
        .write_stdin("# Fresh\nsee [[sot:services/nowhere]]\n")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(text.lines().all(|l| l.starts_with(&format!("{fresh}:"))), "{text}");
    let out = r.spec().args(["check", "--stdin-path", "services/shop/new.md"]).write_stdin("# x\n").output().unwrap();
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(String::from_utf8_lossy(&out.stdout).contains("spec directory"), "{out:?}");
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
fn check_points_at_errors_left_out_by_fail_on() {
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart");
    let p = r.json(&["show", &id]).1["path"].as_str().unwrap().to_string();
    let t = r.read(&p);
    r.write(&p, &format!("{t}\nsee [[sot:services/nowhere]]\n"));
    let (code, v) = r.json(&["check", "--fail-on", "none"]);
    assert_eq!(code, 0, "{v}");
    assert!(v["next"].as_str().unwrap().starts_with("spec explain "), "{v}");
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
fn lsp_offers_replacement_fixes_as_quick_fixes() {
    use serde_json::json;
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart");
    let path = r.json(&["show", &id]).1["path"].as_str().unwrap().to_string();
    let abs = r.path().canonicalize().unwrap();
    let uri = format!("file://{}/{path}", abs.display());
    let text = r.read(&path).replacen(&format!("id: {id}"), &format!("id: {id}x"), 1);
    let line = text.lines().position(|l| l.starts_with("id: ")).unwrap();
    let input = lsp_frames(&[
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"rootUri": format!("file://{}", abs.display())}}),
        json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {"uri": uri, "text": text}}}),
    ]);
    let out = r.spec().arg("lsp").write_stdin(input).output().unwrap();
    let replies = lsp_replies(&out.stdout);
    assert_eq!(replies[0]["result"]["capabilities"]["codeActionProvider"]["codeActionKinds"][0], "quickfix");
    let diags = replies[1]["params"]["diagnostics"].as_array().unwrap().clone();
    let d = diags.iter().find(|d| d.get("data").is_some()).expect("a diagnostic with a fix").clone();
    let input = lsp_frames(&[
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"rootUri": format!("file://{}", abs.display())}}),
        json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {"uri": uri, "text": text}}}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "textDocument/codeAction", "params": {"textDocument": {"uri": uri}, "range": d["range"], "context": {"diagnostics": [d]}}}),
    ]);
    let out = r.spec().arg("lsp").write_stdin(input).output().unwrap();
    let replies = lsp_replies(&out.stdout);
    let action = &replies.iter().find(|m| m["id"] == 2).unwrap()["result"][0];
    assert_eq!(action["kind"], "quickfix");
    let edit = &action["edit"]["changes"][&uri][0];
    assert_eq!(edit["newText"], format!("id: {id}"));
    assert_eq!(edit["range"]["start"], json!({"line": line, "character": 0}));
    assert_eq!(edit["range"]["end"]["character"], format!("id: {id}x").len());
}

#[test]
fn lsp_cached_results_match_fresh_checks() {
    use serde_json::json;
    let r = common::repo();
    let a = r.new_doc("sot", "services/shop", "Cart");
    let b = r.new_doc("sot", "services/shop", "Checkout");
    let abs = r.path().canonicalize().unwrap();
    let uri = |id: &str| format!("file://{}/{}", abs.display(), r.json(&["show", id]).1["path"].as_str().unwrap());
    let (ua, ub) = (uri(&a), uri(&b));
    let pa = r.json(&["show", &a]).1["path"].as_str().unwrap().to_string();
    let ta = r.read(&pa);
    let broken = format!("{ta}\nsee [[{b}]] and [[sot:services/nowhere]]\n");
    let open = |u: &str, t: &str| json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {"uri": u, "text": t}}});
    let change = |u: &str, t: &str| json!({"jsonrpc": "2.0", "method": "textDocument/didChange", "params": {"textDocument": {"uri": u}, "contentChanges": [{"text": t}]}});
    let close =
        |u: &str| json!({"jsonrpc": "2.0", "method": "textDocument/didClose", "params": {"textDocument": {"uri": u}}});
    let pb = r.json(&["show", &b]).1["path"].as_str().unwrap().to_string();
    let tb = r.read(&pb);
    let input = lsp_frames(&[
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"rootUri": format!("file://{}", abs.display())}}),
        open(&ua, &ta),
        open(&ub, &tb),
        open(&ub, &tb),
        change(&ua, &broken),
        open(&ub, &tb),
        change(&ub, &tb.replacen(&format!("id: {b}"), &format!("id: {b}x"), 1)),
        open(&ua, &broken),
        close(&ub),
        open(&ua, &broken),
        change(&ua, &ta),
        json!({"jsonrpc": "2.0", "id": 9, "method": "shutdown"}),
    ]);
    let run = |fresh: bool| {
        let mut c = r.spec();
        c.arg("lsp").env("SPEC_LSP_TRACE", "1");
        if fresh {
            c.env("SPEC_LSP_NO_CACHE", "1");
        }
        c.write_stdin(input.clone()).output().unwrap()
    };
    let (cached, fresh) = (run(false), run(true));
    let (rc, rf) = (lsp_replies(&cached.stdout), lsp_replies(&fresh.stdout));
    assert_eq!(rc.len(), 12, "{rc:?}");
    assert_eq!(rc, rf, "cached and fresh diagnostics differ");
    assert!(rc.iter().any(|m| m["params"]["diagnostics"].as_array().is_some_and(|d| !d.is_empty())));
    let count = |o: &std::process::Output| String::from_utf8_lossy(&o.stderr).matches("full check").count();
    assert_eq!(count(&fresh), 9);
    assert_eq!(count(&cached), 6, "one full check per change of buffers or disk");
}

#[test]
fn check_takes_document_ids_as_well_as_files() {
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart");
    let path = r.json(&["show", &id]).1["path"].as_str().unwrap().to_string();
    let (_, by_id) = r.json(&["check", &id]);
    let (_, by_path) = r.json(&["check", &path]);
    assert_eq!(by_id["findings"], by_path["findings"]);
    let out = r.spec().args(["check", "sot:services/nowhere"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("'sot:services/nowhere' names no document"), "{text}");
}

#[test]
fn check_stdin_errors_name_the_real_problem() {
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart");
    let path = r.json(&["show", &id]).1["path"].as_str().unwrap().to_string();
    let dir = std::path::Path::new(&path).parent().unwrap().to_str().unwrap().to_string();
    // Without --stdin-path the error says nothing about it.
    let out = r.spec().args(["check", "nope.md"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(2));
    assert!(!text.contains("stdin"), "{text}");
    // A buffer whose name breaks the naming: say so and show how to get a usable path.
    let bad = format!("{dir}/notes.md");
    let out = r.spec().args(["check", "--stdin-path", &bad]).write_stdin("# Notes\n").output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(2));
    assert!(text.contains("does not follow this directory's document naming"), "{text}");
    assert!(text.contains("next: spec new sot --title"), "{text}");
    // `-` stands for the --stdin-path buffer, and is explained without it.
    let out = r.spec().args(["check", "--stdin-path", &path, "-"]).write_stdin(r.read(&path)).output().unwrap();
    assert!(matches!(out.status.code(), Some(0 | 1)), "{out:?}");
    let out = r.spec().args(["check", "-"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stdout).contains("spec check --stdin-path <FILE>"), "{out:?}");
}

#[test]
fn lsp_answers_parse_errors_and_explains_broken_frames() {
    let r = common::repo();
    let input = "Content-Length: 5\r\n\r\n{nope".to_string()
        + &lsp_frames(&[serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "shutdown"})]);
    let out = r.spec().arg("lsp").write_stdin(input).output().unwrap();
    let replies = lsp_replies(&out.stdout);
    assert_eq!(replies[0]["error"]["code"], -32700, "{replies:?}");
    assert_eq!(replies[1]["id"], 1, "keeps serving after a parse error");
    for (input, why) in [
        ("Content-Type: x\r\n\r\n{}", "no Content-Length"),
        ("Content-Length: 999999999999\r\n\r\n{}", "exceeds"),
        ("Content-Length: 50\r\n\r\n{}", "inside a 50-byte message body"),
    ] {
        let out = r.spec().arg("lsp").write_stdin(input).output().unwrap();
        assert_eq!(out.status.code(), Some(3), "{input}");
        assert!(String::from_utf8_lossy(&out.stderr).contains(why), "{input}: {out:?}");
    }
}

#[test]
fn lsp_offers_each_candidate_and_runs_only_offered_fix_commands() {
    use serde_json::json;
    let r = common::repo();
    let a = r.new_doc("adr", "services/shop", "First readme");
    let b = r.new_doc("adr", "services/shop", "Second readme");
    for id in [&a, &b] {
        assert_eq!(r.json(&["set", id, "aliases+=README"]).0, 0);
    }
    let c = r.new_doc("sot", "services/shop", "Cart");
    let pc = r.json(&["show", &c]).1["path"].as_str().unwrap().to_string();
    let abs = r.path().canonicalize().unwrap();
    let uri = format!("file://{}/{pc}", abs.display());
    // Ambiguous link in the buffer; a missing key on disk, which `spec fix --rule S1` fills.
    let disk: String = r.read(&pc).lines().filter(|l| !l.starts_with("updated:")).map(|l| format!("{l}\n")).collect();
    std::fs::write(r.path().join(&pc), &disk).unwrap();
    let text = format!("{disk}\nsee [[README]]\n");
    let line = text.lines().position(|l| l.contains("[[README]]")).unwrap();
    let init = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"rootUri": format!("file://{}", abs.display())}});
    let open = json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {"uri": uri, "text": text}}});
    let out = r.spec().arg("lsp").write_stdin(lsp_frames(&[init.clone(), open.clone()])).output().unwrap();
    let replies = lsp_replies(&out.stdout);
    assert_eq!(replies[0]["result"]["capabilities"]["executeCommandProvider"]["commands"][0], "spec.run");
    let diags = replies[1]["params"]["diagnostics"].as_array().unwrap().clone();
    let amb = diags.iter().find(|d| d["code"] == "L1").expect("ambiguous link").clone();
    let s1 = diags.iter().find(|d| d["code"] == "S1").expect("missing key").clone();
    let cmd = s1["data"]["command"].as_str().expect("a runnable fix").to_string();
    let action = |id: u32, d: &serde_json::Value| json!({"jsonrpc": "2.0", "id": id, "method": "textDocument/codeAction", "params": {"textDocument": {"uri": uri}, "range": d["range"], "context": {"diagnostics": [d]}}});
    let run = |id: u32, c: &str| json!({"jsonrpc": "2.0", "id": id, "method": "workspace/executeCommand", "params": {"command": "spec.run", "arguments": [c]}});
    let input = lsp_frames(&[init, open, action(2, &amb), action(3, &s1), run(4, "spec mv x y --write"), run(5, &cmd)]);
    let out = r.spec().arg("lsp").write_stdin(input).output().unwrap();
    let replies = lsp_replies(&out.stdout);
    let get = |id: u32| replies.iter().find(|m| m["id"] == id).unwrap().clone();
    let picks = get(2)["result"].as_array().unwrap().clone();
    assert_eq!(picks.len(), 2, "{picks:?}");
    let mut chosen: Vec<String> = picks
        .iter()
        .map(|p| {
            let e = &p["edit"]["changes"][&uri][0];
            assert_eq!(e["range"]["start"], json!({"line": line, "character": 4}));
            assert_eq!(e["range"]["end"], json!({"line": line, "character": 12}));
            e["newText"].as_str().unwrap().to_string()
        })
        .collect();
    chosen.sort();
    let mut want = vec![format!("[[{a}"), format!("[[{b}")];
    want.sort();
    assert_eq!(chosen, want);
    assert_eq!(get(3)["result"][0]["command"]["arguments"][0], cmd.as_str());
    assert_eq!(get(4)["error"]["code"], -32602, "{:?}", get(4));
    assert_eq!(get(5)["result"]["exit"], 0, "{:?}", get(5));
    assert!(r.read(&pc).lines().any(|l| l.starts_with("updated:")), "S1 fix ran");
}

#[test]
fn hidden_characters_are_reported_and_fixed() {
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart");
    let path = r.json(&["show", &id]).1["path"].as_str().unwrap().to_string();
    let text = format!("{}\nsee\u{200B} [[{id}]] \u{202E}x\u{202C} and \u{200D} emoji\n", r.read(&path));
    std::fs::write(r.path().join(&path), &text).unwrap();
    let (_, v) = r.json(&["check", "--rule", "H1"]);
    let list = v["items"].as_array().unwrap();
    assert_eq!(list.len(), 3, "{v}");
    assert!(list.iter().any(|f| f["message"].as_str().unwrap().contains("U+200B")), "{v}");
    let (code, v) = r.json(&["fix", "--rule", "H1", "--write"]);
    assert_eq!(code, 0, "{v}");
    let (_, v) = r.json(&["check", "--rule", "H1"]);
    assert_eq!(v["warnings"], 0, "{v}");
    assert!(r.read(&path).contains("\u{200D} emoji"), "joiners stay");
}

#[test]
fn names_resolve_in_check_and_typos_get_candidates() {
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart rules");
    let path = r.json(&["show", &id]).1["path"].as_str().unwrap().to_string();
    // check reads names the way resolve does.
    let (code, v) = r.json(&["check", "cart-rules"]);
    assert_eq!(code, 0, "{v}");
    let prefix = &id[..id.len() - 4];
    let (code, v) = r.json(&["check", prefix]);
    assert_eq!(code, 2, "{v}");
    assert_eq!(v["next"], format!("spec show {id}"), "{v}");
    let (_, v) = r.json(&["show", prefix]);
    assert_eq!(v["next"], format!("spec show {id}"), "{v}");
    let (_, v) = r.json(&["check", "nope.md"]);
    assert!(!v["error"].as_str().unwrap().contains("stdin"), "{v}");
    // list and ls are find; an unknown command keeps the generic schema.
    assert_eq!(r.json(&["list"]).1["schema"], "spec.find.v1");
    assert_eq!(r.json(&["ls"]).1["schema"], "spec.find.v1");
    assert_eq!(r.json(&["chek"]).1["schema"], "spec.spec.v1");
    // A status typo and a near-miss path list the likely value first.
    let text = r.read(&path);
    let status = text.lines().find(|l| l.starts_with("status:")).unwrap().to_string();
    let bad = path.replace("cart-rules", "cart-rulez");
    let text = format!("{}\nSee {bad}.\n", text.replace(&status, "status: actve"));
    std::fs::write(r.path().join(&path), text).unwrap();
    let (_, v) = r.json(&["check", "--rule", "S4", "--rule", "L2"]);
    let items = v["items"].as_array().unwrap();
    let s4 = items.iter().find(|f| f["rule"] == "S4").unwrap();
    assert_eq!(s4["candidates"][0], "active", "{v}");
    assert!(s4["message"].as_str().unwrap().contains("did you mean 'active'"), "{v}");
    let l2 = items.iter().find(|f| f["rule"] == "L2").unwrap();
    assert!(l2["message"].as_str().unwrap().contains("no such file"), "{v}");
    assert_eq!(l2["candidates"][0], id.as_str(), "{v}");
    // Nothing to apply: fix points at the first decision, not at --write.
    let (_, v) = r.json(&["fix"]);
    assert!(v["next"].as_str().unwrap().starts_with("spec explain "), "{v}");
}

#[test]
fn machine_paths_in_agent_assets_are_reported() {
    let r = common::repo();
    let toml = r.read("spec.toml") + "[[assets]]\nkind = \"instructions\"\npath = \"AGENTS.md\"\n";
    r.write("spec.toml", &toml);
    r.write("AGENTS.md", "# Agents\nRun /Users/ana/tools/build.sh, not https://example.com/home/x.\n");
    let (_, v) = r.json(&["check", "--rule", "A4"]);
    let items = v["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{v}");
    assert_eq!(items[0]["line"], 2, "{v}");
    assert!(items[0]["message"].as_str().unwrap().contains("/Users/ana"), "{v}");
}

#[test]
fn types_can_require_sections() {
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart");
    let (_, v) = r.json(&["check", "--rule", "S7"]);
    assert_eq!(v["warnings"], 0, "nothing required by default: {v}");
    let toml = r.read("spec.toml") + "[types.sot]\nsections = [\"Rules\", \"Never written here\"]\n";
    r.write("spec.toml", &toml);
    let path = r.json(&["show", &id]).1["path"].as_str().unwrap().to_string();
    r.write(&path, &(r.read(&path) + "\n## rules {#rules}\n"));
    let (_, v) = r.json(&["check", "--rule", "S7"]);
    let items = v["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{v}");
    assert!(items[0]["message"].as_str().unwrap().contains("'Never written here'"), "{v}");
}

#[test]
fn hidden_characters_in_undeclared_agent_files_print_escaped_and_point_at_one_char() {
    use serde_json::json;
    let r = common::repo();
    r.write("AGENTS.md", "# Agents\nrun \u{2066}x\u{2069} and a\u{200B}b\n");
    r.write(".agents/skills/s/SKILL.md", "---\nname: s\ndescription: d\n---\nhi\u{202E}\n");
    let (_, v) = r.json(&["check", "--rule", "H1"]);
    let paths: Vec<&str> = v["items"].as_array().unwrap().iter().map(|f| f["path"].as_str().unwrap()).collect();
    assert_eq!(paths.iter().filter(|p| **p == "AGENTS.md").count(), 3, "{v}");
    assert!(paths.contains(&".agents/skills/s/SKILL.md"), "{v}");
    // Text output escapes what it would otherwise print raw.
    let out = r.spec().args(["check", "--rule", "H1"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(!text.contains('\u{202E}') && !text.contains('\u{2066}'), "{text}");
    assert!(text.contains("remove `\\u{2066}`"), "{text}");
    // The preview shows the lines before and after.
    let out = r.spec().args(["fix", "--rule", "H1"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("update  AGENTS.md:2") && text.contains("  + run x and ab"), "{text}");
    let (_, v) = r.json(&["fix", "--rule", "H1"]);
    let item = v["items"].as_array().unwrap().iter().find(|i| i["path"] == "AGENTS.md").unwrap().clone();
    assert_eq!(item["after"][0], "run x and ab", "{v}");
    // In an editor the diagnostic covers the one character.
    let id = r.new_doc("sot", "services/shop", "Cart");
    let pc = r.json(&["show", &id]).1["path"].as_str().unwrap().to_string();
    let doc = format!("{}\nab\u{200B}c\n", r.read(&pc));
    std::fs::write(r.path().join(&pc), &doc).unwrap();
    let line = doc.lines().count() - 1;
    let abs = r.path().canonicalize().unwrap();
    let uri = format!("file://{}/{pc}", abs.display());
    let init = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"rootUri": format!("file://{}", abs.display())}});
    let open = json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {"uri": uri, "text": doc}}});
    let out = r.spec().arg("lsp").write_stdin(lsp_frames(&[init.clone(), open.clone()])).output().unwrap();
    let replies = lsp_replies(&out.stdout);
    let diags = replies[1]["params"]["diagnostics"].as_array().unwrap().clone();
    let h1 = diags.iter().find(|d| d["code"] == "H1").expect("H1").clone();
    assert_eq!(h1["range"], json!({"start": {"line": line, "character": 2}, "end": {"line": line, "character": 3}}));
    let action = json!({"jsonrpc": "2.0", "id": 2, "method": "textDocument/codeAction", "params": {"textDocument": {"uri": uri}, "range": h1["range"], "context": {"diagnostics": [h1]}}});
    let out = r.spec().arg("lsp").write_stdin(lsp_frames(&[init, open, action])).output().unwrap();
    let replies = lsp_replies(&out.stdout);
    let got = replies.iter().find(|m| m["id"] == 2).unwrap();
    assert_eq!(got["result"][0]["title"], "spec: remove `\\u{200B}`", "{got}");
}

#[test]
fn machine_paths_in_undeclared_agent_files_are_reported_once() {
    let r = common::repo();
    r.write("CLAUDE.md", "# Notes\nrun /Users/ana/bin/tool\n");
    r.write(".cursor/rules/x.mdc", "see /home/dev/repo\n");
    let (_, v) = r.json(&["check", "--rule", "A4"]);
    let mut paths: Vec<&str> = v["items"].as_array().unwrap().iter().map(|f| f["path"].as_str().unwrap()).collect();
    paths.sort();
    assert_eq!(paths, vec![".cursor/rules/x.mdc", "CLAUDE.md"], "{v}");
}

#[test]
fn a_type_table_that_looks_like_a_typo_is_warned_about() {
    let r = common::repo();
    r.write("spec.toml", "[types.adrs]\nsections = [\"Decision\"]\n");
    // Other commands stay quiet about it; check shows it once, check -q not at all.
    let out = r.spec().args(["list"]).output().unwrap();
    assert!(!String::from_utf8_lossy(&out.stderr).contains("[types.adr]"));
    let out = r.spec().args(["check"]).output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(err.matches("did you mean [types.adr]").count(), 1, "{err}");
    let out = r.spec().args(["check", "-q"]).output().unwrap();
    assert!(!String::from_utf8_lossy(&out.stderr).contains("[types.adr]"));
    // doctor lists it once, as a finding.
    let out = r.spec().args(["doctor"]).output().unwrap();
    let all = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert_eq!(all.matches("did you mean [types.adr]").count(), 1, "{all}");
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
fn an_unknown_id_points_at_a_search_not_at_help() {
    let r = common::repo();
    let (code, out) = r.text(&["show", "nope"]);
    assert_ne!(code, 0);
    assert!(out.contains("next: spec find nope"), "{out}");
    assert!(!out.contains("spec help show"), "{out}");
}

#[test]
fn a_unique_timestamp_part_of_an_id_opens_the_document() {
    let r = common::repo();
    let (code, v) = r.json(&["new", "adr", "--title", "Use files"]);
    assert_eq!(code, 0, "{v}");
    let id = v["id"].as_str().unwrap().to_string();
    let stamp = id.rsplit('/').next().unwrap().to_string();
    let (code, d) = r.json(&["show", &stamp]);
    assert_eq!(code, 0, "{d}");
    assert_eq!(d["id"], id.as_str(), "{d}");
    // Part of the timestamp is still only a prefix: it names candidates, it does not open one.
    let (code, text) = r.text(&["show", &stamp[..10]]);
    assert_ne!(code, 0, "{text}");
    assert!(text.contains("starts with it") && text.contains(&id), "{text}");
}
