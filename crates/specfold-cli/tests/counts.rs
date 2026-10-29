//! N1: numbers wrapped in spec:count markers are checked against spec's own counts and fixed.

mod common;

fn path_of(r: &common::Repo, id: &str) -> String {
    r.json(&["resolve", id]).1["path"].as_str().unwrap().to_string()
}

fn append(r: &common::Repo, id: &str, text: &str) -> String {
    let p = path_of(r, id);
    r.write(&p, &(r.read(&p) + text));
    p
}

fn n1(r: &common::Repo) -> Vec<serde_json::Value> {
    let v = r.json(&["check", "--rule", "N1"]).1;
    v["items"].as_array().unwrap().clone()
}

#[test]
fn a_drifted_find_count_is_reported_and_fixed() {
    let r = common::repo();
    r.new_doc("adr", "services/shop", "Use files");
    r.new_doc("adr", "services/shop", "Keep a log");
    let plan = r.new_doc("plan", "services/shop", "Checkout");
    let owner = plan.split_once(':').unwrap().1.rsplit_once('/').unwrap().0;
    let p = append(
        &r,
        &plan,
        &format!(
            "\n결정 <!-- spec:count find type=adr -->1<!-- /spec:count -->개, plans <!-- spec:count find type=plan owner={owner} -->1<!-- /spec:count -->.\n"
        ),
    );
    let items = n1(&r);
    assert_eq!(items.len(), 1, "{items:?}");
    assert_eq!(items[0]["severity"], "warn");
    assert!(items[0]["message"].as_str().unwrap().contains("counts 2"), "{items:?}");
    let (code, preview) = r.json(&["fix", "--rule", "N1"]);
    assert_eq!(code, 0, "{preview}");
    assert!(r.read(&p).contains("-->1<!-- /spec:count -->개"), "preview must not write");
    assert_eq!(r.json(&["fix", "--rule", "N1", "--write"]).0, 0);
    assert!(r.read(&p).contains("결정 <!-- spec:count find type=adr -->2<!-- /spec:count -->개"));
    assert!(n1(&r).is_empty());
}

#[test]
fn table_rows_status_filters_and_code_are_handled() {
    let r = common::repo();
    let plan = r.new_doc("plan", "services/shop", "Checkout");
    append(
        &r,
        &plan,
        "\n## Open items\n\n<!-- spec:count rows=\"Open items\" -->2<!-- /spec:count --> open; \
         <!-- spec:count find type=adr status=active -->0<!-- /spec:count --> active.\n\n\
         | Item | Status |\n|---|---|\n| a | open |\n| b | open |\n\n\
         ```md\n<!-- spec:count find type=adr -->9<!-- /spec:count -->\n```\n\
         Inline `<!-- spec:count find -->9<!-- /spec:count -->` is an example.\n",
    );
    assert!(n1(&r).is_empty(), "{:?}", n1(&r));
}

#[test]
fn malformed_markers_are_errors_without_a_fix() {
    let r = common::repo();
    let plan = r.new_doc("plan", "services/shop", "Checkout");
    append(
        &r,
        &plan,
        "\n<!-- spec:count find type=adr -->3\n\
         <!-- spec:count find --><!-- spec:count find -->1<!-- /spec:count -->\n\
         <!-- spec:count find colour=red -->1<!-- /spec:count -->\n\
         <!-- spec:count rows=Missing -->1<!-- /spec:count -->\n\
         <!-- spec:count run ls -->1<!-- /spec:count -->\n",
    );
    let items = n1(&r);
    assert_eq!(items.len(), 5, "{items:?}");
    assert!(items.iter().all(|f| f["severity"] == "error" && f["fix"].is_null()), "{items:?}");
    let (code, _) = r.json(&["check", "--rule", "N1"]);
    assert_eq!(code, 1);
    let (_, ex) = r.text(&["explain", "N1"]);
    assert!(ex.contains("spec:count"), "{ex}");
}
