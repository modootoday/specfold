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
