//! The CLI contract (C1–C10) checked for every command, so an agent can rely on it.

mod common;

use serde_json::Value;

/// One realistic invocation per command, run against a populated repository.
fn invocations(sot: &str, adr: &str) -> Vec<Vec<String>> {
    let v = |s: &[&str]| s.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    vec![
        v(&["context", "services/shop/src/cart/total.rs"]),
        v(&["find", "cart"]),
        v(&["show", sot]),
        v(&["show", adr, "--tasks"]),
        v(&["resolve", sot]),
        v(&["graph", "impact", sot]),
        v(&["owners"]),
        v(&["log", adr]),
        v(&["new", "plan", "--governs", "services/blog", "--title", "Blog launch", "--dry-run"]),
        v(&["set", adr, "tags+=money", "--dry-run"]),
        v(&["edit", adr, "--section", "Decision", "--text", "Use cards."]),
        v(&["mv", sot, "--slug", "basket"]),
        v(&["fix"]),
        v(&["files", adr]),
        v(&["check"]),
        v(&["verify"]),
        v(&["guard"]),
        v(&["audit"]),
        v(&["census"]),
        v(&["explain", "L1"]),
        v(&["schema", "adr"]),
        v(&["help"]),
        v(&["agents"]),
        v(&["hook", "print"]),
        v(&["plugins"]),
    ]
}

fn populated() -> (common::Repo, String, String) {
    let r = common::repo();
    let sot = r.new_doc("sot", "services/shop/src/cart", "Cart totals");
    let adr = r.new_doc("adr", "services/shop/src/cart", "Round half to even");
    r.commit("docs");
    (r, sot, adr)
}

#[test]
fn c1_every_command_and_argument_is_described() {
    let r = common::repo();
    let (code, v) = r.json(&["help"]);
    assert_eq!(code, 0);
    let items = v["items"].as_array().unwrap();
    assert!(items.len() >= 28, "{} commands", items.len());
    for c in items {
        assert!(!c["about"].as_str().unwrap().is_empty(), "{} has no about", c["name"]);
        for a in c["args"].as_array().unwrap() {
            assert!(!a["help"].as_str().unwrap().is_empty(), "{} --{} has no help", c["name"], a["name"]);
        }
        assert_eq!(c["output"], format!("spec.{}.v1", c["name"].as_str().unwrap()));
    }
}

#[test]
fn c2_c3_c9_json_schema_and_exit_codes_for_every_command() {
    let (r, sot, adr) = populated();
    for args in invocations(&sot, &adr) {
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = r.spec().args(&refs).arg("--json").write_stdin("").output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        let v: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("spec {args:?}: not JSON ({e}): {text}"));
        let name = args[0].as_str();
        assert_eq!(v["schema"], format!("spec.{name}.v1"), "spec {args:?}");
        assert_eq!(
            v["exit"].as_i64().unwrap() as i32,
            out.status.code().unwrap(),
            "spec {args:?}: exit field and code differ"
        );
        assert!(v.get("next").is_some(), "spec {args:?}: no next field");
        assert!(out.status.code().unwrap() < 2, "spec {args:?} failed: {v}");
    }
}

#[test]
fn c4_findings_carry_location_and_fix() {
    let (r, sot, _) = populated();
    let path = r.json(&["resolve", &sot]).1["path"].as_str().unwrap().to_string();
    let text = r.read(&path).replace("created: ", "created: 2026/01/0");
    r.write(&path, &text);
    let (code, v) = r.json(&["check"]);
    assert_eq!(code, 1);
    let f = v["items"].as_array().unwrap().iter().find(|f| f["rule"] == "S5").expect("an S5 finding");
    for k in ["rule", "severity", "path", "line", "message"] {
        assert!(f.get(k).is_some(), "finding lacks {k}");
    }
    assert_eq!(f["path"], path);
}

#[test]
fn c5_writes_are_previews_unless_asked() {
    let (r, sot, _) = populated();
    let before = r.json(&["resolve", &sot]).1["path"].as_str().unwrap().to_string();
    let (code, v) = r.json(&["mv", &sot, "--slug", "basket"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["written"], false);
    assert!(r.path().join(&before).is_file());
    let (code, v) = r.json(&["mv", &sot, "--slug", "basket", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert!(!r.path().join(&before).exists());
    let (code, v) = r.json(&["mv", "sot:shop/basket", "--slug", "basket", "--write"]);
    assert_eq!(code, 0, "moving to where it already is is a no-op: {v}");
    assert_eq!(v["items"].as_array().unwrap().len(), 0);
}

#[test]
fn c6_lists_are_bounded() {
    let (r, _, _) = populated();
    for i in 0..4 {
        r.new_doc("plan", "services/blog", &format!("Plan number {i}"));
    }
    let (_, v) = r.json(&["find", "plan", "--limit", "2"]);
    assert_eq!(v["items"].as_array().unwrap().len(), 2);
    assert_eq!(v["total"], 4);
    assert_eq!(v["more"], 2);
    let (_, t) = r.text(&["find", "plan", "--limit", "2"]);
    assert!(t.contains("+2 more: --offset 2"), "{t}");
}

#[test]
fn c7_one_resolver_for_ids_paths_slugs() {
    let (r, sot, _) = populated();
    let path = r.json(&["resolve", &sot]).1["path"].as_str().unwrap().to_string();
    for q in [sot.as_str(), path.as_str(), "cart-totals"] {
        let (code, v) = r.json(&["resolve", q]);
        assert_eq!(code, 0, "{q}: {v}");
        assert_eq!(v["id"], sot);
    }
    let (code, _) = r.json(&["resolve", "no-such-thing"]);
    assert_eq!(code, 2);
}

#[test]
fn c8_current_documents_by_default() {
    let (r, _, adr) = populated();
    let (code, _) = r.json(&["set", &adr, "status=rejected"]);
    assert_eq!(code, 0);
    assert_eq!(r.json(&["find", "round"]).1["total"], 0);
    assert_eq!(r.json(&["find", "round", "--all"]).1["total"], 1);
}

#[test]
fn c10_text_output_ends_with_next() {
    let (r, sot, _) = populated();
    let (_, t) = r.text(&["show", &sot]);
    assert!(t.trim_end().lines().last().unwrap().starts_with("next: "), "{t}");
}

#[test]
fn usage_errors_exit_2_and_suggest_help() {
    let r = common::repo();
    let (code, v) = r.json(&["nonsense"]);
    assert_eq!(code, 2);
    assert_eq!(v["next"], "spec help");
}
