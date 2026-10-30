//! Agent assets: `spec assets`, `spec audit` and `spec sync` over skills, rules and instructions.

mod common;

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
    assert_eq!(v["written"], false, "deleting needs --yes (CLI-3): {v}");
    assert!(v["next"].as_str().unwrap().ends_with("--prune --write --yes"), "{v}");
    let (code, v) = r.json(&["sync", "assets", "--prune", "--write", "--yes"]);
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
fn sync_says_when_there_are_no_agent_files() {
    let r = common::repo();
    let (_, text) = r.text(&["sync", "assets"]);
    assert!(text.contains("no agent files found"), "{text}");
    r.write("AGENTS.md", "# agents\n");
    let (_, text) = r.text(&["sync", "assets"]);
    assert!(text.contains("agent files found (AGENTS.md)"), "{text}");
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
