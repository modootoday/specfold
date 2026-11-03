//! `spec attach` and `spec check` on registered files.

mod common;

#[test]
fn attach_registers_and_check_catches_changes() {
    let r = common::repo();
    let d = r.new_doc("design", "services/shop", "Cart boards");
    r.write("board.svg", "<svg/>");
    let (code, v) = r.json(&["attach", &d, "board.svg", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(r.json(&["check", "--rule", "P2", "--rule", "E1"]).0, 0);
    let base = r.json(&["files", &d]).1["base"].as_str().unwrap().to_string();
    r.write(&format!("{base}/board.svg"), "<svg>changed</svg>");
    assert_eq!(r.json(&["check", "--rule", "E1"]).0, 1);
    let inside = format!("{base}/board.svg");
    let (code, v) = r.json(&["attach", &d, &inside, "--write"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(r.read(&inside), "<svg>changed</svg>");
    assert_eq!(r.json(&["check", "--rule", "E1"]).0, 0);
}

#[test]
fn attach_stamps_updated_like_new_and_set() {
    let r = common::repo();
    let d = r.new_doc("design", "services/shop", "Cart boards");
    r.write("board.svg", "<svg/>");
    assert_eq!(r.json(&["attach", &d, "board.svg", "--write"]).0, 0);
    let path = r.json(&["show", &d]).1["path"].as_str().unwrap().to_string();
    let text = r.read(&path);
    let updated = text.lines().find_map(|l| l.strip_prefix("updated: ")).expect("updated is written");
    let updated = updated.trim_matches(|c| c == '"' || c == '\'');
    assert!(updated.len() == 20 && updated.contains('T') && updated.ends_with('Z'), "RFC 3339: {updated}");
}

#[test]
fn help_attach_names_the_store_setting() {
    let r = common::repo();
    let (_, t) = r.text(&["help", "attach"]);
    assert!(t.contains("[files.store]") && t.contains("kind = \"dir\""), "{t}");
}

#[test]
fn c202_attach_over_other_content_needs_yes_force_and_keeps_a_copy() {
    let r = common::repo();
    let d = r.new_doc("design", "services/shop", "Cart boards");
    r.write("a/board.svg", "<svg>one</svg>");
    assert_eq!(r.json(&["attach", &d, "a/board.svg", "--write"]).0, 0);
    let base = r.json(&["files", &d]).1["base"].as_str().unwrap().to_string();
    // A registered file with other bytes: preview says replace, the write needs --yes.
    r.write("b/board.svg", "<svg>two</svg>");
    let (code, t) = r.text(&["attach", &d, "b/board.svg"]);
    assert_eq!(code, 0, "{t}");
    assert!(t.contains("replace") && t.contains("--write --yes"), "{t}");
    assert_eq!(r.json(&["attach", &d, "b/board.svg", "--write"]).0, 4);
    assert_eq!(r.read(&format!("{base}/board.svg")), "<svg>one</svg>");
    let (code, v) = r.json(&["attach", &d, "b/board.svg", "--write", "--yes"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(r.read(&format!("{base}/board.svg")), "<svg>two</svg>");
    let undo = v["undo"].as_str().unwrap().to_string();
    assert!(undo.starts_with("cp -Rp '.specfold/trash/") || undo.starts_with("cp -Rp .specfold/trash/"), "{undo}");
    // A hand file the document does not list: refused without --force.
    r.write(&format!("{base}/notes.md"), "MY HAND NOTES");
    r.write("src/notes.md", "other");
    let (code, v) = r.json(&["attach", &d, "src/notes.md", "--write", "--yes"]);
    assert_eq!(code, 2, "{v}");
    assert!(v["next"].as_str().unwrap_or_default().contains("--force"), "{v}");
    assert_eq!(r.read(&format!("{base}/notes.md")), "MY HAND NOTES");
    assert_eq!(r.json(&["attach", &d, "src/notes.md", "--write", "--yes", "--force"]).0, 0);
    assert_eq!(r.read(&format!("{base}/notes.md")), "other");
}

#[test]
fn c225_attach_replace_undo_restores_the_document_too() {
    let r = common::repo();
    let d = r.new_doc("design", "services/shop", "Cart boards");
    r.write("a/board.svg", "<svg>one</svg>");
    assert_eq!(r.json(&["attach", &d, "a/board.svg", "--write"]).0, 0);
    let v = r.json(&["files", &d]).1;
    let base = v["base"].as_str().unwrap().to_string();
    let (_, s) = r.json(&["show", &d]);
    let doc = s["path"].as_str().map(str::to_string).unwrap_or_else(|| format!("{base}/index.md"));
    let before = r.read(&doc);
    r.write("b/board.svg", "<svg>two</svg>");
    let (code, v) = r.json(&["attach", &d, "b/board.svg", "--write", "--yes"]);
    assert_eq!(code, 0, "{v}");
    assert_ne!(r.read(&doc), before, "the attach rewrites the document's files entry");
    let undo = v["undo"].as_str().unwrap();
    let ok = std::process::Command::new("sh").arg("-c").arg(undo).current_dir(r.path()).status().unwrap();
    assert!(ok.success());
    assert_eq!(r.read(&format!("{base}/board.svg")), "<svg>one</svg>");
    assert_eq!(r.read(&doc), before);
}

/// C239: the document's own file, or anything landing on its bundle index name, is refused.
#[test]
fn c239_attach_refuses_the_documents_own_file() {
    let r = common::repo();
    let d = r.new_doc("design", "services/shop", "Cart boards");
    r.write("board.svg", "<svg/>");
    assert_eq!(r.json(&["attach", &d, "board.svg", "--write"]).0, 0);
    let path = r.json(&["show", &d]).1["path"].as_str().unwrap().to_string();
    let before = r.read(&path);
    let (code, v) = r.json(&["attach", &d, &path, "--write"]);
    assert_eq!(code, 2, "{v}");
    assert!(v.to_string().contains("own file"), "{v}");
    let name = std::path::Path::new(&path).file_name().unwrap().to_string_lossy().into_owned();
    r.write(&format!("other/{name}"), "x");
    let (code, v) = r.json(&["attach", &d, &format!("other/{name}"), "--write"]);
    assert_eq!(code, 2, "{v}");
    assert_eq!(r.read(&path), before, "nothing written");
    assert_eq!(r.json(&["check", "--rule", "E1"]).0, 0);
}

fn failing_store(r: &common::Repo) {
    let toml = r.read("spec.toml")
        + "[files]\ninline_max_bytes = 10\n[files.store]\nkind = \"command\"\nput = [\"false\", \"{src}\", \"{key}\"]\nget = [\"false\"]\n";
    r.write("spec.toml", &toml);
}

#[test]
fn c249_c250_a_failed_put_leaves_the_bundle_alone_and_exits_3() {
    let r = common::repo();
    let d = r.new_doc("design", "services/shop", "Cart boards");
    r.write("t2.txt", "small");
    assert_eq!(r.json(&["attach", &d, "t2.txt", "--write"]).0, 0);
    let base = r.json(&["files", &d]).1["base"].as_str().unwrap().to_string();
    let doc = r.json(&["show", &d]).1["path"].as_str().unwrap().to_string();
    let before = r.read(&doc);
    failing_store(&r);
    r.write("t3.txt", "tiny");
    r.write("big2.log", "a file larger than the limit");
    let (code, v) = r.json(&["attach", &d, "t3.txt", "big2.log", "--write", "--yes"]);
    assert_eq!(code, 3, "{v}");
    let e = v["error"].as_str().unwrap();
    assert!(e.contains("store put failed") && e.contains("`false {src} {key}` exited with"), "{e}");
    assert!(!r.path().join(&base).join("big2.log").exists(), "{v}");
    assert!(!r.path().join(&base).join("t3.txt").exists(), "{v}");
    assert_eq!(r.read(&doc), before);
}

#[test]
fn c142_c143_attach_into_stays_inside_the_bundle_and_in_next() {
    let r = common::repo();
    let d = r.new_doc("design", "services/shop", "Boards");
    r.write("a.png", "png");
    r.commit("fixtures");
    for bad in ["../../escape", "/tmp/abs", "x/../../y", "c:/x"] {
        let (code, v) = r.json(&["attach", &d, "a.png", "--into", bad, "--write"]);
        assert_eq!(code, 2, "{bad}: {v}");
    }
    assert!(!r.path().join(".spec/escape").exists());
    let (code, v) = r.json(&["attach", &d, "a.png", "--into", "boards/v1"]);
    assert_eq!(code, 0, "{v}");
    assert!(v["next"].to_string().contains("--into boards/v1"), "{v}");
}

fn find_file(dir: &std::path::Path, name: &str) -> Option<std::path::PathBuf> {
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let p = e.path();
        if p.is_dir() {
            if e.file_name() == ".git" {
                continue;
            }
            if let Some(f) = find_file(&p, name) {
                return Some(f);
            }
        } else if e.file_name().to_string_lossy() == name {
            return Some(p);
        }
    }
    None
}

#[test]
fn c127_fetch_keeps_the_file_when_the_store_holds_other_bytes() {
    let r = common::repo();
    let base = r.read("spec.toml");
    let store = r.path().join("store");
    std::fs::create_dir_all(&store).unwrap();
    r.write("spec.toml", &format!("{base}[files.store]\nkind = \"dir\"\npath = \"{}\"\n", store.display()));
    let d = r.new_doc("design", "services/shop", "Runs");
    r.write("run.log", "ok\n");
    r.commit("fixtures");
    let (code, v) = r.json(&["attach", &d, "run.log", "--store", "--write", "--yes"]);
    assert_eq!(code, 0, "{v}");
    let obj = std::fs::read_dir(&store)
        .unwrap()
        .flatten()
        .flat_map(|e| std::fs::read_dir(e.path()).unwrap().flatten().map(|f| f.path()))
        .next()
        .unwrap();
    std::fs::write(&obj, "tampered\n").unwrap();
    let local = find_file(&r.path().join("services"), "run.log").or_else(|| find_file(r.path(), "run.log"));
    let local = local.filter(|p| !p.ends_with("run.log") || p.parent() != Some(r.path())).unwrap();
    std::fs::write(&local, "mine\n").unwrap();
    let (code, v) = r.json(&["fetch", &d, "--force"]);
    assert_eq!(code, 3, "{v}");
    assert_eq!(std::fs::read_to_string(&local).unwrap(), "mine\n");
    let dir = local.parent().unwrap();
    assert!(
        std::fs::read_dir(dir).unwrap().flatten().all(|e| !e.file_name().to_string_lossy().ends_with(".specfold-part")),
        "no partial file left"
    );
}
