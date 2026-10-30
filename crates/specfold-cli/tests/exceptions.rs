//! Local exceptions: rel.exception_of with a review_by date (rule T5).

mod common;

fn exception() -> (common::Repo, String) {
    let r = common::repo();
    let rule = r.new_doc("guide", "services/shop", "Always use the shared setup");
    let ex = r.new_doc("guide", "services/shop", "Shop keeps its own setup for now");
    let rel = format!("rel.exception_of+={rule}");
    assert_eq!(r.json(&["set", &ex, &rel]).0, 0);
    (r, ex)
}

fn t5(r: &common::Repo) -> Vec<String> {
    let (_, v) = r.json(&["check", "--rule", "T5"]);
    v["items"].as_array().unwrap().iter().map(|f| f["message"].as_str().unwrap().to_string()).collect()
}

#[test]
fn an_exception_needs_a_review_date_and_warns_once_it_passes() {
    let (r, ex) = exception();
    assert!(t5(&r)[0].contains("needs review_by"));
    assert_eq!(r.json(&["set", &ex, "review_by=2999-01-01"]).0, 0);
    assert!(t5(&r).is_empty());
    assert_eq!(r.json(&["check", "--rule", "L3"]).1["errors"], 0);
    assert_eq!(r.json(&["set", &ex, "review_by=2000-01-01"]).0, 0);
    assert!(t5(&r)[0].contains("due for review on 2000-01-01"));
    assert_ne!(r.json(&["set", &ex, "review_by=2000-13-01"]).0, 0);
    let p = r.json(&["resolve", &ex]).1["path"].as_str().unwrap().to_string();
    r.write(&p, &r.read(&p).replace("review_by: 2000-01-01", "review_by: 2000-13-01"));
    assert!(t5(&r)[0].contains("is not a date"), "{:?}", t5(&r));
}

#[test]
fn an_exception_to_nothing_is_a_broken_relation() {
    let r = common::repo();
    let ex = r.new_doc("guide", "services/shop", "Exception");
    assert_eq!(r.json(&["set", &ex, "rel.exception_of+=guide:shop/nope", "review_by=2999-01-01"]).0, 0);
    assert_eq!(r.json(&["check", "--rule", "L3"]).0, 1);
}

#[test]
fn review_by_is_a_known_key_and_t5_can_be_made_an_error() {
    let (r, ex) = exception();
    assert_eq!(r.json(&["set", &ex, "review_by=2000-01-01"]).0, 0);
    assert!(r.json(&["check", "--rule", "S1"]).1["items"].as_array().unwrap().is_empty());
    let cfg = r.read("spec.toml");
    r.write("spec.toml", &format!("{cfg}\n[check.severity]\nT5 = \"error\"\n"));
    let (code, v) = r.json(&["check", "--rule", "T5"]);
    assert_eq!((code, v["errors"].as_u64()), (1, Some(1)), "{v}");
}

#[test]
fn set_refuses_self_exceptions_and_bad_dates_and_audit_lists_overdue_ones() {
    let (r, ex) = exception();
    let (code, v) = r.json(&["set", &ex, &format!("rel.exception_of+={ex}")]);
    assert_ne!(code, 0, "{v}");
    for bad in ["review_by=2026-13-40", "review_by=tomorrow"] {
        let (code, v) = r.json(&["set", &ex, bad]);
        assert_ne!(code, 0, "{v}");
        assert!(v.to_string().contains("YYYY-MM-DD"), "{v}");
    }
    let (code, v) = r.json(&["set", &ex, "review_by=2000-01-01"]);
    assert_eq!(code, 0, "{v}");
    assert!(v["next"].as_str().unwrap_or_default().contains("--rule T5"), "{v}");
    assert!(t5(&r)[0].contains("status rejected or superseded"));
    let (_, v) = r.json(&["audit", "--kind", "exception"]);
    assert_eq!(v["items"][0]["subject"], ex.as_str(), "{v}");
    let p = r.json(&["resolve", &ex]).1["path"].as_str().unwrap().to_string();
    let text = r.read(&p);
    let line = text.lines().find(|l| l.contains("exception_of")).unwrap().to_string();
    r.write(&p, &text.replacen(&line, &format!("  exception_of: [{ex}]"), 1));
    assert!(t5(&r).iter().any(|m| m.contains("names this document itself")), "{:?}", t5(&r));
}
