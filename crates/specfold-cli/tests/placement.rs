//! Where `spec new` puts a document in a repository that mounts owners and projects as
//! submodules, and what P1 says about it.

mod common;

/// A git repository with one commit and no spec.toml.
fn plain_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let git = |args: &[&str]| {
        let ok = std::process::Command::new("git").args(args).current_dir(dir.path()).output().unwrap();
        assert!(ok.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&ok.stderr));
    };
    git(&["init", "-q", "-b", "main"]);
    std::fs::create_dir_all(dir.path().join("infra/backend")).unwrap();
    std::fs::write(dir.path().join("infra/backend/main.rs"), "fn main() {}\n").unwrap();
    git(&["add", "-A"]);
    git(&["-c", "user.email=t@example.com", "-c", "user.name=t", "commit", "-qm", "init"]);
    dir
}

fn mount(r: &common::Repo, url: &std::path::Path, at: &str) {
    r.git(&["-c", "protocol.file.allow=always", "submodule", "add", "-q", url.to_str().unwrap(), at]);
}

/// The id and path `spec new plan` would create, from the repository root or `-C dir`.
fn preview(r: &common::Repo, dir: Option<&str>, governs: &[&str]) -> (String, String) {
    let mut args: Vec<&str> = dir.map(|d| vec!["-C", d]).unwrap_or_default();
    args.extend(["new", "plan", "--title", "t"]);
    for g in governs {
        args.extend(["--governs", g]);
    }
    let (code, v) = r.json(&args);
    assert_eq!(code, 0, "{v}");
    (v["id"].as_str().unwrap().to_string(), v["path"].as_str().unwrap().to_string())
}

/// A repository with an owner kind `apps/*`: a plain-directory owner, an owner mounted as a
/// submodule without spec.toml, one mounted with its own spec.toml, and an outside project.
fn workspace() -> (common::Repo, Vec<tempfile::TempDir>) {
    let r = common::repo();
    r.write(
        "spec.toml",
        &r.read("spec.toml").replace("[public]", "[[owners.kinds]]\npattern = \"apps/*\"\nid = \"{1}\"\n[public]"),
    );
    r.write("apps/plain/infra/backend/main.rs", "fn main() {}\n");
    let brand = plain_repo();
    mount(&r, brand.path(), "apps/brand");
    let hub = common::repo();
    hub.write("infra/backend/main.rs", "fn main() {}\n");
    hub.commit("hub code");
    mount(&r, hub.path(), "apps/hub");
    let outside = common::repo();
    mount(&r, outside.path(), "vendor/outside");
    r.commit("mount");
    (r, vec![brand, hub.dir, outside.dir])
}

#[test]
fn a1_governs_in_a_submodule_owner_places_the_document_with_that_owner() {
    let (r, _keep) = workspace();
    let cases = [
        ("apps/plain/infra/backend", "plan:plain/", "apps/plain/.spec/plan/"),
        ("apps/brand/infra/backend", "plan:brand/", "apps/brand/.spec/plan/"),
        ("apps/hub/infra/backend", "plan:hub/", "apps/hub/.spec/plan/"),
        ("vendor/outside/services/shop", "plan:ws/", ".spec/plan/"),
    ];
    for (governs, id, path) in cases {
        let (got_id, got_path) = preview(&r, None, &[governs]);
        assert!(got_id.starts_with(id) && got_path.starts_with(path), "{governs}: {got_id} {got_path}");
    }
}

#[test]
fn a1_p1_asks_a_root_document_governing_a_submodule_owner_to_move() {
    let (r, _keep) = workspace();
    let (code, v) = r.json(&["new", "plan", "--title", "t", "--governs", "apps/brand/infra/backend", "--owner", "ws"]);
    assert_ne!(code, 0, "--owner ws contradicts the governs: {v}");
    assert!(v.to_string().contains("belongs to brand"), "{v}");
    let id = r.new_doc("plan", "vendor/outside/services/shop", "Outside code");
    assert_eq!(r.json(&["set", &id, "rel.governs+=apps/brand/infra/backend", "--write"]).0, 0);
    let (_, v) = r.json(&["check", "--rule", "P1"]);
    assert_eq!(v["errors"], 1, "{v}");
    assert!(v.to_string().contains("spec mv") && v.to_string().contains("--owner brand"), "{v}");
}

#[test]
fn a2_dash_c_makes_the_owner_of_that_directory_the_current_owner() {
    let (r, _keep) = workspace();
    let (id, path) = preview(&r, Some("apps/brand"), &[]);
    assert!(id.starts_with("plan:brand/") && path.starts_with("apps/brand/.spec/plan/"), "{id} {path}");
    let (id, path) = preview(&r, Some("apps/brand"), &["infra/backend"]);
    assert!(id.starts_with("plan:brand/") && path.starts_with("apps/brand/.spec/plan/"), "{id} {path}");
    let (_, v) = r.json(&["-C", "apps/brand", "new", "plan", "--title", "t", "--governs", "infra/backend"]);
    assert_eq!(v["owner_reason"], "governs code inside apps/brand", "{v}");
    let (id, _) = preview(&r, Some("apps/plain/infra"), &[]);
    assert!(id.starts_with("plan:plain/"), "{id}");
    let (id, _) = preview(&r, Some("services"), &[]);
    assert!(id.starts_with("plan:ws/"), "a directory no owner holds stays with the root: {id}");
}

const PICK: &str = "---\nid: adr:ws/20260101000000\ntitle: Pick\nstatus: active\ncreated: 2026-01-01\nupdated: 2026-01-01\n---\n\n# Pick\n\n## Context\n\n## Decision\n\n## Consequences\n";

/// A repository mounting an independent project at packages/cli whose root id and document ids
/// equal the outer repository's, published, with a broken citation and a denied word inside.
fn nested_repo(nested: bool) -> common::Repo {
    let r = common::repo();
    let mut toml = r.read("spec.toml").replace(
        "[public]\npaths = [\"libs/open\"]",
        "[[owners.kinds]]\npattern = \"packages/*\"\nid = \"pkg.{1}\"\n[public]\npaths = [\"libs/open\", \"packages/cli\"]",
    );
    if nested {
        toml = toml.replace("root = \"ws\"\n", "root = \"ws\"\nnested = [\"packages/cli\"]\n");
    }
    toml.push_str("[cites]\npaths = [\"**/*.md\"]\n");
    toml.push_str("[[guards]]\nname = \"published\"\npublic = true\nrange = \"all\"\ndeny_terms = [\"hushword\"]\n");
    toml.push_str("[[guards]]\nname = \"inner\"\nnested = true\nrange = \"all\"\ndeny_terms = [\"hushword\"]\n");
    r.write("spec.toml", &toml);
    r.write(".spec/adr/20260101000000-pick.adr.md", PICK);
    r.write("packages/cli/spec.toml", "[spec]\nschema = 1\n[owners]\nroot = \"ws\"\n");
    r.write("packages/cli/.spec/adr/20260101000000-pick.adr.md", PICK);
    r.write("packages/cli/src/main.rs", "fn main() {}\n");
    r.write("packages/cli/README.md", "See [[adr:gone/20260101000000]]. The hushword.\n");
    r.commit("nested");
    r
}

#[test]
fn c1_a_nested_project_is_not_an_owner_and_root_rules_leave_it_alone() {
    let r = nested_repo(false);
    let (code, v) = r.json(&["check"]);
    assert_eq!(code, 1, "without the declaration its documents clash and break rules: {v}");
    let r = nested_repo(true);
    let (code, v) = r.json(&["check"]);
    assert_eq!((code, v["errors"].as_u64()), (0, Some(0)), "{v}");
    let owners = r.json(&["owners"]).1.to_string();
    assert!(!owners.contains("pkg.cli"), "{owners}");
    let (code, v) = r.json(&["show", "cli:adr:ws/20260101000000"]);
    assert_eq!(code, 0, "the nested project is the remote cli: {v}");
    let id = r.new_doc("plan", "packages/cli/src", "Use the cli");
    assert!(id.starts_with("plan:ws/"), "governs into a nested project are another project's: {id}");
    assert_eq!(r.json(&["check", "--rule", "L5", "--rule", "P1"]).1["warnings"], 0);
}

#[test]
fn c2_guards_name_nested_projects_by_reference() {
    let r = nested_repo(true);
    let (code, v) = r.json(&["guard", "published", "--tree"]);
    assert_eq!(code, 0, "public leaves nested projects to their own checks: {v}");
    let (code, v) = r.json(&["guard", "inner", "--tree"]);
    assert_eq!(code, 1, "{v}");
    assert!(v.to_string().contains("packages/cli/README.md"), "{v}");
    let toml = r.read("spec.toml").replace("nested = [\"packages/cli\"]", "nested = [\"packages/*\"]");
    r.write("spec.toml", &toml);
    let (code, v) = r.json(&["check"]);
    assert_eq!(code, 2, "{v}");
    assert!(v.to_string().contains("[owners] nested"), "{v}");
}
