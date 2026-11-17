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

/// A workspace with a root document, a document of another owner and an alias, and a repository
/// it mounts at `apps/<dir>` with one document citing all three.
struct Mounted {
    ws: common::Repo,
    brand: common::Repo,
    doc: String,
    path: String,
}

fn in_workspace() -> Mounted {
    let ws = common::repo();
    let root = ws.new_doc("sot", "spec.toml", "Workspace layout");
    let sibling = ws.new_doc("sot", "services/blog", "Blog rules");
    let plan = ws.new_doc("plan", "services/shop", "Old plan");
    ws.write(".spec/_aliases.tsv", &format!("# old\tnew\nplan/old-plan.md\t{plan}\n"));
    std::fs::create_dir_all(ws.path().join("apps")).unwrap();
    let brand = common::Repo::new(tempfile::tempdir_in(ws.path().join("apps")).unwrap());
    let mount = format!("apps/{}", brand.path().file_name().unwrap().to_str().unwrap());
    brand.git(&["init", "-q", "-b", "main"]);
    brand.write("spec.toml", &format!("[spec]\nschema = 1\n[owners]\nroot = \"brand\"\nmount = \"{mount}\"\n"));
    let doc = brand.new_doc("sot", &mount, "Brand rules");
    let path = brand.json(&["resolve", &doc]).1["path"].as_str().unwrap().to_string();
    brand.write(&path, &(brand.read(&path) + &format!("\n[[{root}]] [[{sibling}]] [[plan/old-plan.md]]\n")));
    assert_eq!(brand.json(&["set", &doc, &format!("rel.depends_on+={sibling}"), "--write"]).0, 0);
    assert_eq!(brand.json(&["set", &doc, "rel.depends_on+=plan/old-plan.md", "--write"]).0, 0);
    Mounted { ws, brand, doc, path }
}

fn errors(r: &common::Repo) -> (u64, serde_json::Value) {
    let (_, v) = r.json(&["check", "--rule", "L1", "--rule", "L3"]);
    (v["errors"].as_u64().unwrap(), v)
}

#[test]
fn a_mounted_repository_resolves_workspace_sibling_and_alias_targets_in_its_parent() {
    let m = in_workspace();
    let (n, v) = errors(&m.brand);
    assert_eq!(n, 0, "{v}");
}

#[test]
fn a_target_the_mounting_workspace_lacks_is_still_an_error() {
    let m = in_workspace();
    m.brand.write(&m.path, &(m.brand.read(&m.path) + "\n[[plan:ws/19990101000000]]\n"));
    assert_eq!(m.brand.json(&["set", &m.doc, "rel.depends_on+=nowhere", "--write"]).0, 0);
    let (n, v) = errors(&m.brand);
    assert_eq!(n, 2, "{v}");
    assert!(v.to_string().contains("[[plan:ws/19990101000000]] names no document"), "{v}");
    assert!(v.to_string().contains("target nowhere names no document"), "{v}");
}

#[test]
fn without_the_mounting_workspace_on_disk_its_targets_are_skipped() {
    let m = in_workspace();
    m.brand.write(&m.path, &(m.brand.read(&m.path) + "\n[[plan:ws/19990101000000]]\n"));
    std::fs::rename(m.ws.path().join("spec.toml"), m.ws.path().join("spec.toml.off")).unwrap();
    let (n, v) = errors(&m.brand);
    assert_eq!(n, 0, "{v}");
}
