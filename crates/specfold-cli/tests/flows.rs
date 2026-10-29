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
