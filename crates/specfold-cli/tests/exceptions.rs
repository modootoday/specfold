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
    assert_eq!(r.json(&["set", &ex, "review_by=2000-13-01"]).0, 0);
    assert!(t5(&r)[0].contains("is not a date"));
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
