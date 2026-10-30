//! Rules a project shapes through spec.toml, and the verify runner's verdicts.

mod common;

fn path_of(r: &common::Repo, id: &str) -> String {
    r.json(&["resolve", id]).1["path"].as_str().unwrap().to_string()
}

fn edit(r: &common::Repo, id: &str, from: &str, to: &str) {
    let p = path_of(r, id);
    let text = r.read(&p);
    assert!(text.contains(from), "{text}");
    r.write(&p, &text.replacen(from, to, 1));
}

fn rules_hit(v: &serde_json::Value) -> Vec<String> {
    v["items"].as_array().unwrap().iter().map(|f| f["rule"].as_str().unwrap().to_string()).collect()
}

#[test]
fn a_custom_type_moves_through_its_own_statuses_and_requires_fields() {
    let r = common::repo();
    let toml = r.read("spec.toml")
        + "[types.finding]\nkey = \"stamp\"\nstatuses = [\"open\", \"resolved\", \"accepted-risk\"]\ninitial = \"open\"\nfields = [\"review_by\"]\n[types.finding.transitions]\nopen = [\"resolved\", \"accepted-risk\"]\n[types.finding.require_when]\naccepted-risk = [\"review_by\"]\n";
    r.write("spec.toml", &toml);
    let id = r.new_doc("finding", "services/shop", "Token in logs");
    assert_eq!(r.json(&["set", &id, "status=accepted-risk"]).0, 0);
    let (code, v) = r.json(&["check", "--rule", "S6"]);
    assert_eq!(code, 1, "{v}");
    assert_eq!(r.json(&["set", &id, "review_by=2026-12-01"]).0, 0);
    assert_eq!(r.json(&["check", "--rule", "S6", "--rule", "S4"]).0, 0);
    let (code, v) = r.json(&["set", &id, "status=open"]);
    assert_ne!(code, 0, "accepted-risk may not go back to open: {v}");
}

#[test]
fn a_supersede_cycle_is_reported() {
    let r = common::repo();
    let a = r.new_doc("adr", "services/shop", "Use files");
    let b = r.new_doc("adr", "services/shop", "Use a database");
    for (doc, other) in [(&a, &b), (&b, &a)] {
        edit(&r, doc, "status: draft", "status: superseded");
        edit(&r, doc, "rel:\n", &format!("rel:\n  supersedes:\n  - \"{other}\"\n"));
    }
    let (code, v) = r.json(&["check", "--rule", "T1"]);
    assert_eq!(code, 1, "{v}");
    assert!(v.to_string().contains("supersede cycle"), "{v}");
}

fn break_id(r: &common::Repo, id: &str) {
    edit(r, id, &format!("id: {id}"), "id: sot:shop/wrong");
}

#[test]
fn a_baseline_accepts_the_known_pile_and_fails_anything_new() {
    let r = common::repo();
    let toml = r.read("spec.toml") + "[check]\nbaseline = \"spec-baseline.json\"\nstrict = [\"services/blog/**\"]\n";
    r.write("spec.toml", &toml);
    let old = r.new_doc("sot", "services/shop/src", "Old rule");
    break_id(&r, &old);
    let (code, v) = r.json(&["check", "--write-baseline", "--yes"]);
    assert_ne!(code, 0, "a dirty tree must not be measured: {v}");
    r.commit("old pile");
    let (code, v) = r.json(&["check", "--write-baseline", "--yes"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["baselined"], 1);
    r.commit("baseline");
    let (code, v) = r.json(&["check", "--rule", "S2"]);
    assert_eq!((code, v["baselined"].as_u64()), (0, Some(1)), "{v}");
    let strict = r.new_doc("sot", "services/blog/src", "Blog rule");
    break_id(&r, &strict);
    let (code, v) = r.json(&["check", "--rule", "S2"]);
    assert_eq!(code, 1, "a strict path is never held by the baseline: {v}");
    assert_eq!(r.json(&["check", "--rule", "S2", "--no-baseline"]).1["errors"], 2);
    let (code, _) = r.json(&["check", "--fail-on", "NOPE"]);
    assert_ne!(code, 0);
    let (code, v) = r.json(&["check", "--rule", "S2", "--fail-on", "L1"]);
    assert_eq!(code, 0, "only L1 decides the exit code here: {v}");
}

#[test]
fn a_hand_edit_must_move_updated() {
    let r = common::repo();
    let id = r.new_doc("plan", "services/shop", "Ship it");
    let p = path_of(&r, &id);
    let updated = r.json(&["show", &id]).1["frontmatter"]["updated"].as_str().unwrap().to_string();
    r.write(&p, &r.read(&p).replace(&format!("updated: {updated}"), "updated: 2026-01-02"));
    r.commit("old plan");
    r.write(&p, &(r.read(&p) + "\nOne more step.\n"));
    r.git(&["add", "-A"]);
    let (_, v) = r.json(&["check", "--staged", "--rule", "T4"]);
    assert_eq!(rules_hit(&v), ["T4"], "{v}");
    assert_eq!(r.json(&["fix", "--rule", "T4", "--write", "--staged"]).0, 0);
    r.git(&["add", "-A"]);
    assert!(rules_hit(&r.json(&["check", "--staged", "--rule", "T4"]).1).is_empty());
}

#[test]
fn citations_in_code_resolve_and_staged_runs_read_only_added_lines() {
    let r = common::repo();
    r.write("spec.toml", &(r.read("spec.toml") + "[cites]\npaths = [\"services/**/*.rs\", \"AGENTS.md\"]\n"));
    let sot = r.new_doc("sot", "services/shop/src", "Cart totals");
    let sot_path = path_of(&r, &sot);
    r.write(
        "services/shop/src/cart/total.rs",
        &format!(
            "// see [[{sot}]]\n// old [[sot:shop/gone]]\n// path {sot_path}\nfn total() {{}}\n// [[allowlist]] if [[ -f x ]]\n"
        ),
    );
    r.write("AGENTS.md", "Rules moved to sot:shop/gone long ago; formerly [[sot:shop/gone]].\n");
    let (code, v) = r.json(&["check", "--rule", "L6"]);
    assert_eq!(code, 1, "{v}");
    let messages: Vec<String> = v["items"].as_array().unwrap().iter().map(|f| f["message"].to_string()).collect();
    assert_eq!(messages.len(), 2, "{messages:?}");
    assert!(
        messages.iter().any(|m| m.contains("names no document")) && messages.iter().any(|m| m.contains("is a path"))
    );
    r.commit("cites");
    let text = r.read("services/shop/src/cart/total.rs") + "// new [[sot:shop/also-gone]]\n";
    r.write("services/shop/src/cart/total.rs", &text);
    r.git(&["add", "-A"]);
    let (_, v) = r.json(&["check", "--staged", "--rule", "L6"]);
    let items = v["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{v}");
    assert_eq!(items[0]["line"], 6);
}

#[test]
fn published_paths_are_detected_from_manifests() {
    let r = common::repo();
    r.write("spec.toml", &r.read("spec.toml").replace("[public]\n", "[public]\ndetect = [\"npm-files\"]\n"));
    r.new_doc("sot", "services/shop/src", "Cart");
    r.write("services/shop/package.json", "{\"name\": \"shop\"}\n");
    let (code, v) = r.json(&["check", "--rule", "V1"]);
    assert_eq!(code, 1, "a package with no files list ships its spec directory: {v}");
    assert!(v.to_string().contains("has no files list"), "{v}");
    r.write("services/shop/package.json", "{\"name\": \"shop\", \"files\": [\"dist\"]}\n");
    assert_eq!(r.json(&["check", "--rule", "V1"]).0, 0);
    r.write("services/shop/package.json", "{\"name\": \"shop\", \"files\": [\".\"]}\n");
    assert_eq!(r.json(&["check", "--rule", "V1"]).0, 1);
}

#[test]
fn a_visibility_manifest_older_than_gitmodules_is_reported() {
    let r = common::repo();
    r.write(".gitmodules", "[submodule \"services/blog\"]\n\tpath = services/blog\n\turl = x\n");
    r.write(
        "vis.json",
        "{\"gitmodulesSha256\": \"0\", \"submodules\": {\"services/blog\": {\"visibility\": \"PUBLIC\"}}}\n",
    );
    let toml = r.read("spec.toml").replace("[public]\n", "[public]\nsubmodules_manifest = \"vis.json\"\n");
    r.write("spec.toml", &toml);
    r.new_doc("plan", "services/shop", "Ship");
    let blog_doc = r.path().join("services/blog/.spec/research/20260101000000-x.research.md");
    std::fs::create_dir_all(blog_doc.parent().unwrap()).unwrap();
    std::fs::write(&blog_doc, "---\nid: research:blog/20260101000000\n---\n# X\n").unwrap();
    let (code, v) = r.json(&["check", "--rule", "V1"]);
    assert_eq!(code, 1, "{v}");
    let text = v.to_string();
    assert!(text.contains("older than .gitmodules") && text.contains("public submodule"), "{text}");
}

#[test]
fn severity_turns_a_rule_down_or_off() {
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop/src", "Rule");
    break_id(&r, &id);
    r.write("spec.toml", &(r.read("spec.toml") + "[check.severity]\nS2 = \"warn\"\n"));
    let (code, v) = r.json(&["check", "--rule", "S2"]);
    assert_eq!((code, v["warnings"].as_u64()), (0, Some(1)), "{v}");
    r.write("spec.toml", &r.read("spec.toml").replace("S2 = \"warn\"", "S2 = \"off\""));
    assert_eq!(r.json(&["check", "--rule", "S2"]).1["warnings"], 0);
}

#[test]
fn a_pending_owner_keeps_its_old_layout_unread_until_import() {
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop/src", "Imported early");
    r.write("services/shop/.spec/sots/old-rule.md", "# old\n");
    assert_eq!(rules_hit(&r.json(&["check", "--rule", "S3"]).1), ["S3"]);
    r.write(
        "spec.toml",
        &r.read("spec.toml").replace("root = \"ws\"\n", "root = \"ws\"\npending = [\"services/shop\"]\n"),
    );
    assert!(rules_hit(&r.json(&["check", "--rule", "S3"]).1).is_empty());
    assert_eq!(r.json(&["show", &id]).0, 0);
}

#[test]
fn a_staged_gitlink_brings_the_files_changed_inside_it_into_the_run() {
    let lib = common::repo();
    lib.write("docs/readme.md", "lib\n");
    lib.commit("lib");
    let r = common::repo();
    r.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        "-q",
        lib.path().to_str().unwrap(),
        "services/vendor",
    ]);
    r.commit("pin vendor");
    r.write("services/vendor/.spec/notes/handoff.md", "# a note\n");
    let sub = r.path().join("services/vendor");
    for args in [
        vec!["add", ".spec/notes/handoff.md"],
        vec!["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-qm", "note"],
    ] {
        assert!(std::process::Command::new("git").args(&args).current_dir(&sub).status().unwrap().success());
    }
    r.git(&["add", "services/vendor"]);
    assert_eq!(rules_hit(&r.json(&["check", "--staged", "--rule", "S3"]).1), ["S3"]);
    assert_eq!(rules_hit(&r.json(&["check", "--changed", "HEAD", "--rule", "S3"]).1), ["S3"]);
}

#[test]
fn a_full_check_leaves_whole_tree_guards_to_spec_guard() {
    let r = common::repo();
    let toml = r.read("spec.toml")
        + "[[guards]]\nname = \"g\"\npaths = [\"services/**\"]\nrange = \"all\"\ndeny_terms = [\"acme-internal\"]\n";
    r.write("spec.toml", &toml);
    r.write("services/shop/notes.txt", "acme-internal\n");
    assert!(rules_hit(&r.json(&["check", "--rule", "U1"]).1).is_empty());
    assert_eq!(rules_hit(&r.json(&["check", "--rule", "U1", "--guards"]).1), ["U1"]);
    r.git(&["add", "-A"]);
    assert_eq!(rules_hit(&r.json(&["check", "--staged", "--rule", "U1"]).1), ["U1"]);
}

#[test]
fn a_whole_tree_guard_reads_tracked_files_an_ignore_file_matches() {
    let r = common::repo();
    let toml = r.read("spec.toml")
        + "[[guards]]\nname = \"g\"\npaths = [\"libs/open/**\"]\nrange = \"all\"\ndeny_terms = [\"acme-internal\"]\n";
    r.write("spec.toml", &toml);
    r.write("libs/open/.gitignore", "/*\n!/.gitignore\n!/docs/\n");
    r.write("libs/open/docs/a.md", "clean\n");
    r.write("libs/open/forced.md", "acme-internal\n");
    r.write("libs/open/stray.md", "acme-internal\n");
    r.git(&["add", "libs/open/.gitignore", "libs/open/docs/a.md"]);
    r.git(&["add", "-f", "libs/open/forced.md"]);
    r.commit("forced");
    let (code, v) = r.json(&["guard", "g", "--all"]);
    assert_eq!(code, 1, "{v}");
    let paths: Vec<String> =
        v["items"].as_array().unwrap().iter().map(|f| f["path"].as_str().unwrap_or("").to_string()).collect();
    assert_eq!(paths, ["libs/open/forced.md"], "{v}");
}

#[cfg(unix)]
#[test]
fn verify_reports_timeouts_missing_programs_and_undeclared_checks() {
    let r = common::repo();
    let slow = r.new_doc("sot", "services/shop/src", "Slow rule");
    edit(&r, &slow, "status: draft", "status: active\nverify:\n  - sleep 5\nverify_timeout_ms: 300");
    let tool = r.new_doc("sot", "services/shop/src/cart", "Needs a tool");
    edit(&r, &tool, "status: draft", "status: active\nverify:\n  - no-such-tool-xyz --check");
    let manual = r.new_doc("sot", "services/blog/src", "Checked by people");
    edit(&r, &manual, "status: draft", "status: active\nverify: manual");
    let bare = r.new_doc("sot", "services/blog", "Says nothing");
    edit(&r, &bare, "status: draft", "status: active");
    let (code, v) = r.json(&["verify"]);
    assert_eq!(code, 1, "{v}");
    let text = v.to_string();
    assert!(text.contains("timed out") && text.contains("cannot run"), "{text}");
    assert_eq!(v["failed"], 2, "{v}");
    let (_, v) = r.json(&["check", "--rule", "X2"]);
    let hits: Vec<String> =
        v["items"].as_array().unwrap().iter().map(|f| f["path"].as_str().unwrap().to_string()).collect();
    assert_eq!(hits, [path_of(&r, &bare)], "{v}");
    assert_eq!(rules_hit(&v), ["X2"]);
}

#[test]
fn an_unknown_key_shows_how_to_declare_it_and_declaring_it_quiets_s1() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Keep a domain");
    edit(&r, &id, "\nstatus:", "\ndomain: billing\nstatus:");
    let (_, v) = r.json(&["check", "--rule", "S1"]);
    let msg = v["items"][0]["message"].as_str().unwrap_or_default().to_string();
    assert!(msg.contains("[types.adr] fields = [") && msg.contains("\"domain\""), "{v}");
    r.write("spec.toml", &(r.read("spec.toml") + "[types.adr]\nfields = [\"domain\"]\n"));
    let (code, v) = r.json(&["check", "--rule", "S1"]);
    assert_eq!(code, 0, "{v}");
    assert!(rules_hit(&v).is_empty(), "{v}");
}

#[test]
fn agent_assets_need_a_name_and_description_and_warn_on_the_rest() {
    let r = common::repo();
    let toml = r.read("spec.toml")
        + "[[assets]]\nkind = \"skill\"\npath = \".agents/skills\"\n[[assets]]\nkind = \"skill\"\npath = \".claude/skills\"\n[[assets]]\nkind = \"rule\"\npath = \".cursor/rules\"\n";
    r.write("spec.toml", &toml);
    r.write(".agents/skills/pdf-tools/SKILL.md", "---\nname: pdf-tools\ndescription: Read PDFs.\n---\n# PDF\n");
    let (code, v) = r.json(&["check", "--rule", "A1", "--rule", "A2"]);
    assert_eq!(code, 0, "a valid skill passes: {v}");
    r.write(".agents/skills/broken/SKILL.md", "---\nname: Broken\nDescription: x\n---\n");
    r.write(".claude/skills/one/SKILL.md", "---\nname: same\ndescription: a\n---\n");
    r.write(".claude/skills/two/SKILL.md", "---\nname: same\ndescription: b\n---\n");
    r.write(".cursor/rules/style.md", "Use tabs.\n");
    let (code, v) = r.json(&["check", "--rule", "A1", "--rule", "A2"]);
    assert_eq!(code, 1, "{v}");
    let text = v.to_string();
    assert_eq!(v["errors"], 1, "only the missing description is an error: {v}");
    assert!(text.contains("skill has no description"), "{v}");
    assert!(text.contains("the field is spelled 'description'"), "{v}");
    assert!(text.contains("differs from its directory 'broken'"), "{v}");
    assert!(text.contains("lowercase letters"), "{v}");
    assert!(text.contains("skill name 'same' is also used by .claude/skills/"), "{v}");
    assert!(text.contains("Cursor reads only .mdc"), "{v}");
    assert!(!text.contains("pdf-tools"), "the valid skill stays quiet: {v}");
}

#[test]
fn agent_assets_name_paths_and_spec_commands_that_exist() {
    let r = common::repo();
    let toml = r.read("spec.toml")
        + "[[assets]]\nkind = \"instructions\"\npath = \"AGENTS.md\"\n[[assets]]\nkind = \"skill\"\npath = \".agents/skills\"\n";
    r.write("spec.toml", &toml);
    r.write(
        ".agents/skills/cart/SKILL.md",
        "---\nname: cart\ndescription: Cart.\n---\nSee [the guide](guide.md) and `services/shop/src/cart/total.rs`.\n",
    );
    r.write(".agents/skills/cart/guide.md", "guide\n");
    r.write(
        "AGENTS.md",
        "# Agents\n\nRun `spec check --staged`, read `services/shop/src/cart/old.rs` and [setup](docs/setup.md).\nIds like `sot:shop/cart` and globs like `src/**/*.rs` are not paths. See https://example.com/a/b.\n\n```sh\nspec show <id>\nspec frobnicate\n```\n",
    );
    let (code, v) = r.json(&["check", "--rule", "A3"]);
    assert_eq!(code, 0, "{v}");
    let msgs: Vec<String> =
        v["items"].as_array().unwrap().iter().map(|i| i["message"].as_str().unwrap().to_string()).collect();
    assert_eq!(msgs.len(), 3, "{msgs:?}");
    assert!(msgs.iter().any(|m| m.contains("names services/shop/src/cart/old.rs")), "{msgs:?}");
    assert!(msgs.iter().any(|m| m.contains("links to docs/setup.md")), "{msgs:?}");
    assert!(msgs.iter().any(|m| m.contains("`spec frobnicate`")), "{msgs:?}");
    assert_eq!(v["errors"], 0, "warnings only: {v}");
}

#[test]
fn s8_a_required_field_needs_a_real_value_of_its_kind() {
    let r = common::repo();
    let toml = r.read("spec.toml")
        + "[types.finding]\nkey = \"stamp\"\nstatuses = [\"open\", \"accepted-risk\"]\ninitial = \"open\"\nfields = { review_by = \"date\", approvers = \"people\", authors = \"people\", decision = \"link\" }\ndistinct = [[\"authors\", \"approvers\"]]\n[types.finding.transitions]\nopen = [\"accepted-risk\"]\n[types.finding.require_when]\naccepted-risk = [\"review_by\", \"approvers\", \"decision\"]\n";
    r.write("spec.toml", &toml);
    let sot = r.new_doc("sot", "services/shop", "Logs");
    let id = r.new_doc("finding", "services/shop", "Token in logs");
    // A placeholder on a status that does not require the field is fine.
    assert_eq!(r.json(&["set", &id, "approvers=TBD"]).0, 0);
    let (_, v) = r.json(&["check", "--rule", "S8"]);
    assert!(rules_hit(&v).is_empty(), "{v}");
    for kv in ["status=accepted-risk", "review_by=2026-12-01", "authors=ann", "decision=sot:nowhere"] {
        let (c, v) = r.json(&["set", &id, kv]);
        assert_eq!(c, 0, "{kv}: {v}");
    }
    let (code, v) = r.json(&["check", "--rule", "S8"]);
    assert_eq!(code, 0, "S8 warns by default: {v}");
    let s = v.to_string();
    assert!(s.contains("'TBD' is a placeholder"), "{v}");
    assert!(s.contains("names no document"), "{v}");
    assert!(s.contains(&format!("spec set {id} approvers=")), "{v}");
    assert_eq!(r.json(&["set", &id, "approvers=Ann"]).0, 0);
    let (_, v) = r.json(&["check", "--rule", "S8"]);
    assert!(v.to_string().contains("in both authors and approvers"), "{v}");
    for kv in ["authors=Bob", &format!("decision={sot}")] {
        assert_eq!(r.json(&["set", &id, kv]).0, 0, "{kv}");
    }
    let (_, v) = r.json(&["check", "--rule", "S8", "--rule", "S1"]);
    assert!(rules_hit(&v).is_empty(), "declared kinds are known keys and the values pass: {v}");
}

#[test]
fn l8_l9_local_links_fragments_and_alt_text() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Use files");
    let p = path_of(&r, &id);
    let dir = p.rsplit_once('/').unwrap().0.to_string();
    r.write(&format!("{dir}/pic.png"), "png");
    let body = "\n## Use files\n## Use files\n### Why {#why-1}\n\n[ok](#use-files-1) [ok](#why-1) ![Shown](pic.png) [web](https://x.invalid/a#b)\n[gone](missing.md) [frag](#nope) ![](pic.png)\n`[code](missing2.md)`\n";
    r.write(&p, &(r.read(&p) + body));
    let (_, v) = r.json(&["check", "--rule", "L8", "--rule", "L9"]);
    let mut hit = rules_hit(&v);
    hit.sort();
    assert_eq!(hit, ["L8", "L8", "L9"], "{v}");
    assert_eq!(v["errors"], 0, "both warn: {v}");
    let toml = r.read("spec.toml");
    let toml = if toml.lines().any(|l| l == "[check]") {
        toml.replacen("[check]\n", "[check]\nlink_ignore = [\"missing.*\"]\n", 1)
    } else {
        format!("{toml}\n[check]\nlink_ignore = [\"missing.*\"]\n")
    };
    r.write("spec.toml", &toml);
    let (_, v) = r.json(&["check", "--rule", "L8"]);
    assert_eq!(rules_hit(&v), ["L8"], "link_ignore skips missing.md, #nope stays: {v}");
}

#[test]
fn c109_c110_c111_c112_set_lists_statuses_and_nexts() {
    let r = common::repo();
    let toml = r.read("spec.toml")
        + "[types.adr.require_when]\nactive = [\"approvers\", \"review_by\"]\naccepted = [\"approvers\"]\n";
    r.write("spec.toml", &toml);
    let id = r.new_doc("adr", "services/shop", "Use files");
    let out = r.spec().args(["check"]).output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("status 'accepted'") && err.contains("draft"), "C110: {err}");
    let (_, v) = r.json(&["set", &id, "status=active"]);
    let next = v["next"].to_string();
    assert!(next.contains("S6") && next.contains("S8"), "C111: {v}");
    let (_, v) = r.json(&["check", "--rule", "S6"]);
    let fix = v["items"][0].to_string();
    assert!(fix.contains("approvers=") && fix.contains("review_by="), "C112: every missing field: {v}");
    r.json(&["set", &id, "approvers=[kim, lee]"]);
    let (_, d) = r.json(&["show", &id]);
    assert_eq!(d["frontmatter"]["approvers"], serde_json::json!(["kim", "lee"]), "C109: {d}");
}

#[test]
fn sd3_coverage_lists_anchors_no_other_document_cites() {
    let r = common::repo();
    let sot = r.new_doc("sot", "services/shop", "Login");
    let (_, d) = r.json(&["show", &sot]);
    let path = d["path"].as_str().unwrap().to_string();
    r.write(&path, &(r.read(&path) + "\n## Password login {#fr-001}\n\n## Magic link {#fr-002}\n"));
    assert_eq!(r.json(&["set", &sot, "status=active", "--write"]).0, 0);
    let plan = r.new_doc("plan", "services/shop", "Login work");
    let (_, p) = r.json(&["show", &plan]);
    let ppath = p["path"].as_str().unwrap().to_string();
    r.write(&ppath, &(r.read(&ppath) + &format!("\n- [ ] build [[{sot}#fr-001]]\n")));
    r.commit("docs");
    // Off by default: plain sots get no noise.
    let (_, v) = r.json(&["audit", "--kind", "coverage"]);
    assert!(v["items"].as_array().is_none_or(|a| a.is_empty()), "{v}");
    r.write("spec.toml", &(r.read("spec.toml") + "[audit]\ncoverage_types = [\"sot\"]\n"));
    let (_, v) = r.json(&["audit", "--kind", "coverage"]);
    let items = v["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{v}");
    assert!(items[0]["detail"].as_str().unwrap().contains("#fr-002"), "{v}");
}
