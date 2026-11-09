//! `-C <dir>`: paths, hints and undo commands resolve against the directory it names.

#[path = "../common/mod.rs"]
mod common;

/// Runs `spec -C .. <args> --json` from `sub/`, where a decoy of every input sits.
fn from_sub(r: &common::Repo, args: &[&str]) -> (i32, serde_json::Value) {
    let sub = r.path().join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    let mut c = r.spec();
    c.current_dir(&sub).arg("-C").arg("..").args(args).arg("--json");
    let o = c.output().unwrap();
    (o.status.code().unwrap_or(-1), serde_json::from_slice(&o.stdout).unwrap_or_default())
}

#[test]
fn c161_dir_resolves_attach_files() {
    let r = common::repo();
    let d = r.new_doc("sot", "services/shop", "Cart");
    r.write("note.txt", "inside\n");
    r.write("sub/note.txt", "outside\n");
    r.commit("doc");
    let (code, v) = from_sub(&r, &["attach", &d, "note.txt"]);
    assert_eq!(code, 0, "{v}");
    assert!(v["next"].as_str().unwrap().starts_with("spec -C .. attach"), "{v}");
    let (code, v) = from_sub(&r, &["attach", &d, "note.txt", "--write"]);
    assert_eq!(code, 0, "{v}");
    let (_, v) = r.json(&["show", &d]);
    let dir = v.to_string();
    assert!(dir.contains("note.txt"), "{v}");
    let found = walk_all(r.path())
        .into_iter()
        .find(|p| p.ends_with("note.txt") && !p.starts_with(r.path().join("sub")) && *p != r.path().join("note.txt"));
    assert_eq!(std::fs::read_to_string(found.expect("attached copy")).unwrap(), "inside\n");
}

fn walk_all(p: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = vec![];
    for e in std::fs::read_dir(p).unwrap().flatten() {
        let q = e.path();
        if q.file_name().is_some_and(|n| n == ".git") {
            continue;
        }
        if q.is_dir() { out.extend(walk_all(&q)) } else { out.push(q) }
    }
    out
}

#[test]
fn c161_dir_resolves_export_out() {
    let r = common::repo();
    r.new_doc("sot", "services/shop", "Cart");
    r.commit("doc");
    let (code, v) = from_sub(&r, &["export", "markdown", "--target", "obsidian", "--out", "outdir"]);
    assert_eq!(code, 0, "{v}");
    assert!(r.path().join("outdir").is_dir(), "{v}");
    assert!(!r.path().join("sub/outdir").exists(), "{v}");
}

#[test]
fn c161_dir_resolves_import_map() {
    let (r, _) = common::madr_repo(true);
    let sub = r.path().join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("import.toml"), "not = [toml").unwrap();
    let (code, v) = from_sub(&r, &["import", "--map", "import.toml", "--dry-run"]);
    assert_eq!(code, 0, "{v}");
    assert!(v["rows"].as_u64().unwrap() > 0, "{v}");
}

#[test]
fn c163_dir_reaches_shell_next_undo_and_fix_hints() {
    let r = common::repo();
    let d = r.new_doc("sot", "services/shop", "Cart");
    r.commit("doc");
    let (code, v) = from_sub(&r, &["set", &d, "title=Basket", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert!(v["undo"].as_str().unwrap().starts_with("git -C .. restore -- "), "{v}");
    let (code, v) = from_sub(&r, &["export", "markdown", "--target", "obsidian", "--out", "outdir"]);
    assert_eq!(code, 0, "{v}");
    let next = v["next"].as_str().unwrap();
    assert!(next.starts_with("git -C .. status") || next.starts_with("cd .. && "), "{v}");
    // A check fix hint keeps -C too.
    // The document by its own path: the export above wrote a copy whose name also holds "cart".
    let p = r.path().join(common::doc_path(&r, &d));
    let t = std::fs::read_to_string(&p).unwrap();
    std::fs::write(&p, t.replacen(&d, &format!("{d}x"), 1)).unwrap();
    let mut c = r.spec();
    c.current_dir(r.path().join("sub")).args(["-C", "..", "check"]);
    let out = String::from_utf8(c.output().unwrap().stdout).unwrap();
    assert!(out.contains("(spec -C .. fix --write") || out.contains("fix: spec -C .. "), "{out}");
    let (_, plain) = r.text(&["check"]);
    assert!(!plain.contains("-C"), "{plain}");
}
