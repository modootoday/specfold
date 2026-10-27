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
    let (code, v) = r.json(&["check", "--write-baseline"]);
    assert_ne!(code, 0, "a dirty tree must not be measured: {v}");
    r.commit("old pile");
    let (code, v) = r.json(&["check", "--write-baseline"]);
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
        &format!("// see [[{sot}]]\n// old [[sot:shop/gone]]\n// path {sot_path}\nfn total() {{}}\n"),
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
    assert_eq!(items[0]["line"], 5);
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
