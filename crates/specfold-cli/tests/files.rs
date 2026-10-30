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
