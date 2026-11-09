//! External sources (V3): ids and backticked repository paths, per source, with paths inside a
//! gitlink read from the commit it pins.

#[path = "../common/mod.rs"]
mod common;

use serde_json::Value;

fn found(v: &Value) -> Vec<(String, u64, String)> {
    v["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            let s = |k: &str| f[k].as_str().unwrap().to_string();
            (s("path"), f["line"].as_u64().unwrap(), s("message"))
        })
        .collect()
}

fn git_in(dir: &std::path::Path, args: &[&str]) {
    let ok = std::process::Command::new("git")
        .args(["-c", "user.email=t@t", "-c", "user.name=t", "-c", "protocol.file.allow=always"])
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap()
        .success();
    assert!(ok, "git {args:?}");
}

const NOTE: &str = "# Memory\n\
Cart total lives in `services/shop/src/cart/total.rs`; the old `services/shop/src/gone.rs` moved.\n\
Decided in adr:ws/20990101000000, see also [[adr:ws/20990101000001]] and {ID}.\n\
```\nservices/none/in/a/fence.rs `services/none/either.rs`\n```\n";

fn memory(r: &common::Repo, flags: &str) -> (tempfile::TempDir, String) {
    let id = r.new_doc("adr", "services/shop", "Cart totals");
    let mem = tempfile::tempdir().unwrap();
    std::fs::write(mem.path().join("note.md"), NOTE.replace("{ID}", &id)).unwrap();
    let toml = format!("[external.memory]\npath = \"{}\"\n{flags}", mem.path().display());
    r.write("spec.toml", &(r.read("spec.toml") + &toml));
    (mem, id)
}

#[test]
fn v3_ids_and_paths_are_checked_per_source_without_external() {
    let r = common::repo();
    let (_mem, _) = memory(&r, "ids = true\ncheck_paths = true\n");
    let (code, v) = r.json(&["check", "--rule", "V3"]);
    assert_eq!(code, 0, "warnings only: {v}");
    let f = found(&v);
    let msgs: Vec<&str> = f.iter().map(|(_, _, m)| m.as_str()).collect();
    assert_eq!(
        msgs,
        [
            "names services/shop/src/gone.rs, which does not exist",
            "names adr:ws/20990101000000, which is no document here",
            "names adr:ws/20990101000001, which is no document here",
        ],
        "{v}"
    );
    assert!(f.iter().all(|(p, _, _)| p == "memory:note.md"), "{v}");
}

#[test]
fn v3_new_checks_are_off_unless_a_source_sets_them() {
    let r = common::repo();
    let (_mem, _) = memory(&r, "");
    let (code, v) = r.json(&["check", "--rule", "V3"]);
    assert_eq!((code, v["warnings"].as_u64()), (0, Some(0)), "{v}");
    let (_, v) = r.json(&["check", "--rule", "V3", "--external"]);
    let msgs: Vec<String> = found(&v).into_iter().map(|(_, _, m)| m).collect();
    assert_eq!(msgs, ["[[adr:ws/20990101000001]] names no document"], "--external keeps its citation check: {v}");
}

#[test]
fn v3_a_path_inside_a_gitlink_counts_when_the_pinned_commit_holds_it() {
    let lib = common::Repo::new(tempfile::tempdir().unwrap());
    git_in(lib.path(), &["init", "-q", "-b", "main"]);
    lib.write("src/a.rs", "fn a() {}\n");
    git_in(lib.path(), &["add", "-A"]);
    git_in(lib.path(), &["commit", "-qm", "lib"]);
    let r = common::repo();
    git_in(r.path(), &["submodule", "add", "-q", lib.path().to_str().unwrap(), "vendor/lib"]);
    r.write("AGENTS.md", "# Agents\nUse `vendor/lib/src/a.rs`, not `vendor/lib/src/zz.rs`.\n");
    r.write("spec.toml", &(r.read("spec.toml") + "[external.agents]\npath = \"AGENTS.md\"\ncheck_paths = true\n"));
    r.commit("agents");
    std::fs::remove_file(r.path().join("vendor/lib/src/a.rs")).unwrap();
    let (_, v) = r.json(&["check", "--rule", "V3"]);
    assert_eq!(found(&v), [("agents:AGENTS.md".into(), 2, "names vendor/lib/src/zz.rs, which does not exist".into())]);
}

#[test]
fn v3_a_staged_run_checks_a_changed_source_in_the_repository() {
    let r = common::repo();
    r.write("AGENTS.md", "# Agents\n");
    r.write("spec.toml", &(r.read("spec.toml") + "[external.agents]\npath = \"AGENTS.md\"\nids = true\n"));
    r.commit("agents");
    r.write("AGENTS.md", "# Agents\nFollow adr:ws/20990101000000.\n");
    let (_, v) = r.json(&["check", "--staged", "--rule", "V3"]);
    assert_eq!(v["warnings"], 0, "an unstaged change is not checked: {v}");
    r.git(&["add", "AGENTS.md"]);
    let (_, v) = r.json(&["check", "--staged", "--rule", "V3"]);
    assert_eq!(found(&v).len(), 1, "{v}");
}
