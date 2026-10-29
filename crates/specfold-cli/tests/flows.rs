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
