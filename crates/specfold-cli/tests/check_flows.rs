//! `spec check` flows: staged and named files, stdin buffers, summaries and fixes.

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
    let out = r.spec().arg("lsp").write_stdin(common::lsp_frames(&[init.clone(), open.clone()])).output().unwrap();
    let replies = common::lsp_replies(&out.stdout);
    let diags = replies[1]["params"]["diagnostics"].as_array().unwrap().clone();
    let h1 = diags.iter().find(|d| d["code"] == "H1").expect("H1").clone();
    assert_eq!(h1["range"], json!({"start": {"line": line, "character": 2}, "end": {"line": line, "character": 3}}));
    let action = json!({"jsonrpc": "2.0", "id": 2, "method": "textDocument/codeAction", "params": {"textDocument": {"uri": uri}, "range": h1["range"], "context": {"diagnostics": [h1]}}});
    let out = r.spec().arg("lsp").write_stdin(common::lsp_frames(&[init, open, action])).output().unwrap();
    let replies = common::lsp_replies(&out.stdout);
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
fn the_commit_check_refuses_a_document_secret_with_no_guard_configured() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Database");
    let path = r.json(&["show", &id]).1["path"].as_str().unwrap().to_string();
    let url = format!("postgres://app:{}{}@db.acme.io/main", "s3cr", "Et-77x");
    r.write(&path, &(r.read(&path) + "\nDSN: " + &url + "\n"));
    r.git(&["add", "-A"]);
    let (code, v) = r.json(&["check", "--staged"]);
    assert_eq!(code, 1, "{v}");
    let s = v.to_string();
    assert!(s.contains("\"U2\"") && s.contains(&path) && !s.contains("Et-77x"), "{s}");
    let (code, v) = r.json(&["guard", "secrets", "--staged"]);
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["next"], "spec explain U2", "{v}");
    let (code, v) = r.json(&["guard"]);
    assert_eq!(code, 1, "spec guard with no guards configured runs the built-in check: {v}");
}

#[test]
fn a_secret_in_a_local_document_is_a_warning() {
    let r = common::repo();
    let (code, v) = r.json(&["new", "adr", "--governs", "services/shop", "--title", "Notes", "--local"]);
    assert_eq!(code, 0, "{v}");
    let id = v["id"].as_str().unwrap().to_string();
    let path = r.json(&["show", &id]).1["path"].as_str().unwrap().to_string();
    let url = format!("postgres://app:{}{}@db.acme.io/main", "s3cr", "Et-77x");
    r.write(&path, &(r.read(&path) + "\nDSN: " + &url + "\n"));
    let (code, v) = r.json(&["check", "--rule", "U2"]);
    assert_eq!(code, 0, "warnings do not fail the check: {v}");
    let s = v.to_string();
    assert!(s.contains("\"U2\"") && s.contains("warn") && !s.contains("Et-77x"), "{s}");
    let (code, v) = r.json(&["guard", "secrets"]);
    assert_eq!(code, 0, "{v}");
}

#[test]
fn a_secret_in_a_repository_attachment_is_found_after_attach() {
    let r = common::repo();
    let (code, v) = r.json(&["new", "design", "--governs", "services/shop", "--title", "Board"]);
    assert_eq!(code, 0, "{v}");
    let id = v["id"].as_str().unwrap().to_string();
    let url = format!("postgres://app:{}{}@db.acme.io/main", "s3cr", "Et-77x");
    r.write("notes.txt", &format!("DSN: {url}\n"));
    let mut bin = b"\0\x01PNG".to_vec();
    bin.extend(format!("x {}{}\n", "postgres://app:s3crEt-88y", "@db.acme.io/main").bytes());
    std::fs::write(r.path().join("board.bin"), &bin).unwrap();
    let (code, v) = r.json(&["attach", &id, "notes.txt", "board.bin", "--write"]);
    assert_eq!(code, 0, "a repository file only warns at attach time: {v}");
    let (code, v) = r.json(&["check", "--rule", "U2"]);
    assert_ne!(code, 0, "the attachment is read by every later check: {v}");
    let s = v.to_string();
    assert!(s.contains("notes.txt") && !s.contains("Et-77x"), "{s}");
    assert!(!s.contains("board.bin"), "binary files are skipped: {s}");
    let (code, v) = r.json(&["guard", "secrets", "--all"]);
    assert_ne!(code, 0, "{v}");
    assert!(v.to_string().contains("notes.txt"), "{v}");
    r.write("spec.toml", &(r.read("spec.toml") + "\n[files]\nscan_max_bytes = 8\n"));
    let (code, v) = r.json(&["check", "--rule", "U2"]);
    assert_eq!(code, 0, "over scan_max_bytes the file is not read: {v}");
}
