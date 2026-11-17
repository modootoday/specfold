//! A repository a larger workspace mounts (`[owners] mount`): governs are written from the
//! workspace root (L10), and L5 finds them from this repository's root.

#[path = "../common/mod.rs"]
mod common;

fn rules_hit(v: &serde_json::Value) -> Vec<String> {
    v["items"].as_array().unwrap().iter().map(|f| f["rule"].as_str().unwrap().to_string()).collect()
}

fn mounted(at: &str) -> common::Repo {
    let r = common::repo();
    let toml = r.read("spec.toml").replacen("root = \"ws\"\n", &format!("root = \"ws\"\nmount = \"{at}\"\n"), 1);
    r.write("spec.toml", &toml);
    r
}

#[test]
fn l10_governs_written_from_the_repository_is_an_error_with_a_fix() {
    let r = mounted("apps/shop");
    r.new_doc("plan", "services/shop", "Local form");
    let (code, v) = r.json(&["check", "--rule", "L10"]);
    assert_eq!(code, 1, "{v}");
    assert_eq!(rules_hit(&v), ["L10"], "{v}");
    assert!(v.to_string().contains("rel.governs+=apps/shop/services/shop"), "{v}");
}

#[test]
fn l10_governs_under_the_mount_passes() {
    let r = mounted("apps/shop");
    r.new_doc("plan", "apps/shop/services/shop", "Workspace form");
    r.new_doc("plan", "apps/shop", "The whole repository");
    let (code, v) = r.json(&["check", "--rule", "L10"]);
    assert_eq!((code, rules_hit(&v)), (0, vec![]), "{v}");
}

#[test]
fn l10_without_a_mount_reports_nothing() {
    let r = common::repo();
    r.new_doc("plan", "services/shop", "Local form");
    let (code, v) = r.json(&["check", "--rule", "L10"]);
    assert_eq!((code, rules_hit(&v)), (0, vec![]), "{v}");
}

#[test]
fn l5_reads_a_mounted_governs_from_the_repository_root() {
    let r = mounted("apps/shop");
    r.new_doc("plan", "apps/shop/services/shop/src/**", "Existing code");
    let (_, v) = r.json(&["check", "--rule", "L5"]);
    assert!(rules_hit(&v).is_empty(), "{v}");
    r.new_doc("plan", "apps/shop/services/gone", "Missing code");
    let (_, v) = r.json(&["check", "--rule", "L5"]);
    assert_eq!(rules_hit(&v), ["L5"], "{v}");
    assert!(v.to_string().contains("services/gone does not exist"), "{v}");
}

#[test]
fn a_mount_with_a_parent_step_or_a_root_is_refused() {
    for bad in ["../shop", "/apps/shop", "apps/*", "apps/../shop", ""] {
        let r = mounted(bad);
        let (code, out) = r.text(&["check"]);
        assert_eq!(code, 2, "mount {bad:?}: {out}");
    }
}
