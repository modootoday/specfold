//! Project boundaries: what N1 counts and the census see across a nested project, a vendored
//! submodule, a remote checked out inside the repository and a public clone mapped in roots.toml.

#[path = "../common/mod.rs"]
mod common;

use serde_json::Value;

const MARKER: &str = "<!-- spec:count find type=adr -->7<!-- /spec:count --> decisions.\n";

/// Files every project carries: a README with a wrong count and a file a retired layout forbids.
fn project_files(r: &common::Repo, dir: &str) {
    let at = |f: &str| if dir.is_empty() { f.to_string() } else { format!("{dir}/{f}") };
    r.write(&at("README.md"), &format!("# Project\n\n{MARKER}"));
    r.write(&at("LEGACY.txt"), "retired\n");
}

/// A repository with a nested project at packages/cli, a vendored submodule at vendor/child and the
/// remote `near` checked out at third_party/near, each carrying `project_files` (the nested project
/// only when `nested_files`).
fn bounded_repo(nested_files: bool) -> (common::Repo, common::Repo) {
    let child = common::repo();
    project_files(&child, "");
    child.commit("child");
    let r = common::repo();
    let url = child.path().to_str().unwrap();
    r.git(&["-c", "protocol.file.allow=always", "submodule", "add", "-q", url, "vendor/child"]);
    let toml = r.read("spec.toml").replace("root = \"ws\"\n", "root = \"ws\"\nnested = [\"packages/cli\"]\n")
        + "[check]\ncount_paths = [\"**/README.md\"]\n[forbid]\npaths = [\"**/LEGACY.txt\"]\n"
        + "[remotes.near]\npath = \"third_party/near\"\n";
    r.write("spec.toml", &toml);
    r.new_doc("adr", "services/shop", "Use files");
    project_files(&r, "");
    for dir in ["packages/cli", "third_party/near"] {
        r.write(&format!("{dir}/spec.toml"), "[spec]\nschema = 1\n[owners]\nroot = \"ws\"\n");
        if nested_files || dir != "packages/cli" {
            project_files(&r, dir);
        }
    }
    r.commit("projects");
    (r, child)
}

fn finding_paths(v: &Value) -> Vec<String> {
    let mut p: Vec<String> =
        v["items"].as_array().unwrap().iter().map(|i| i["path"].as_str().unwrap().into()).collect();
    p.sort();
    p
}

#[test]
fn n1_count_paths_reach_into_submodules_and_in_repo_remotes() {
    let (r, _child) = bounded_repo(false);
    let (_, v) = r.json(&["check", "--rule", "N1"]);
    assert_eq!(finding_paths(&v), ["README.md", "third_party/near/README.md", "vendor/child/README.md"], "{v}");
}

#[test]
fn census_legacy_paths_count_submodules_and_in_repo_remotes() {
    let (r, _child) = bounded_repo(false);
    let (_, v) = r.json(&["census"]);
    assert_eq!(v["legacy_paths"], 3, "the root, the submodule and the remote each hold one: {v}");
}

#[test]
fn z1_leaves_nested_projects_out_and_reaches_submodules_and_remotes() {
    let (r, _child) = bounded_repo(true);
    let (_, v) = r.json(&["check", "--rule", "Z1"]);
    assert_eq!(finding_paths(&v), ["LEGACY.txt", "third_party/near/LEGACY.txt", "vendor/child/LEGACY.txt"], "{v}");
}

#[test]
fn nested_projects_are_left_out_of_n1_the_census_assets_and_v3() {
    let (r, _child) = bounded_repo(true);
    let toml = r.read("spec.toml")
        + "[[assets]]\nkind = \"instructions\"\npath = \"**/AGENTS.md\"\n[external.notes]\npath = \"packages\"\n";
    r.write("spec.toml", &toml);
    for dir in ["", "packages/cli/"] {
        r.write(&format!("{dir}AGENTS.md"), "# Agents\n");
    }
    for dir in ["packages/cli", "packages/notes"] {
        r.write(&format!("{dir}/NOTES.md"), "See [[adr:gone/20260101000000]].\n");
    }
    r.commit("assets and notes");
    let (_, v) = r.json(&["check", "--rule", "N1"]);
    assert_eq!(finding_paths(&v), ["README.md", "third_party/near/README.md", "vendor/child/README.md"], "{v}");
    let (_, v) = r.json(&["census"]);
    assert_eq!(v["legacy_paths"], 3, "{v}");
    let (_, v) = r.json(&["assets"]);
    let assets: Vec<&str> = v["items"].as_array().unwrap().iter().map(|i| i["path"].as_str().unwrap()).collect();
    assert_eq!(assets, ["AGENTS.md"], "{v}");
    let (_, v) = r.json(&["check", "--rule", "V3", "--external"]);
    assert_eq!(finding_paths(&v), ["notes:notes/NOTES.md"], "{v}");
}

#[test]
fn a_public_clone_mapped_in_roots_toml_is_neither_counted_nor_censused() {
    let private = common::repo();
    let toml = private.read("spec.toml")
        + "[check]\ncount_paths = [\"**/README.md\"]\n[forbid]\npaths = [\"**/LEGACY.txt\"]\n";
    private.write("spec.toml", &toml);
    private.new_doc("adr", "services/shop", "Use files");
    private.commit("counts");
    let public = tempfile::tempdir().unwrap();
    let clone = public.path().join("open");
    std::fs::create_dir_all(&clone).unwrap();
    assert!(std::process::Command::new("git").args(["init", "-q"]).current_dir(&clone).status().unwrap().success());
    std::fs::write(clone.join("README.md"), format!("# Open\n\n{MARKER}")).unwrap();
    std::fs::write(clone.join("LEGACY.txt"), "retired\n").unwrap();
    let roots = public.path().join("roots.toml");
    let text = format!("[[root]]\nrepo = \"{}\"\nspec = \"{}\"\n", clone.display(), private.path().display());
    std::fs::write(&roots, text).unwrap();
    let run = |args: &[&str]| {
        let out =
            private.spec().current_dir(&clone).env("SPECFOLD_ROOTS", &roots).args(args).arg("--json").output().unwrap();
        serde_json::from_slice::<Value>(&out.stdout).unwrap()
    };
    assert!(finding_paths(&run(&["check", "--rule", "N1"])).is_empty(), "the clone's README is not read");
    assert_eq!(run(&["census"])["legacy_paths"], 0, "the clone's files are not walked");
}
