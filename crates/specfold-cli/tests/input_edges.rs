//! Input edge cases from the evaluation rounds: values that start with `-`, `--text -`, files
//! that are not UTF-8, the checks `set aliases=` points at, slugs that drop words, and errors
//! whose next step is a real command (C74-C79).

mod common;

#[test]
fn values_may_start_with_a_hyphen() {
    let r = common::repo();
    let (code, v) = r.json(&["new", "adr", "--governs", "services/shop", "--title", "-1 retries"]);
    assert_eq!(code, 0, "{v}");
    let id = v["id"].as_str().unwrap().to_string();
    assert!(r.json(&["show", &id]).1.to_string().contains("-1 retries"));
    let (code, v) = r.json(&["edit", &id, "--section", "Decision", "--text", "- [ ] wire gateway", "--write"]);
    assert_eq!(code, 0, "{v}");
    let path = r.json(&["show", &id]).1["path"].as_str().unwrap().to_string();
    let s = r.read(&path);
    assert!(s.contains("- [ ] wire gateway"), "{s}");
}

#[test]
fn text_dash_is_refused_with_the_stdin_flag() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Pick a gateway");
    let before = r.text(&["show", &id]).1;
    let (code, v) = r.json(&["edit", &id, "--section", "Decision", "--text", "-", "--write"]);
    assert_ne!(code, 0, "{v}");
    assert!(v.to_string().contains("--file -"), "{v}");
    assert_eq!(v["next"], format!("spec edit {id} --section Decision --file - --write"));
    assert_eq!(r.text(&["show", &id]).1, before, "nothing written");
}

#[test]
fn a_file_that_is_not_utf8_is_named_with_its_line() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Pick a gateway");
    let path = r.json(&["show", &id]).1["path"].as_str().unwrap().to_string();
    let mut bytes = std::fs::read(r.path().join(&path)).unwrap();
    let lines_before = bytes.iter().filter(|b| **b == b'\n').count();
    bytes.extend_from_slice(b"bad \xff byte\n");
    std::fs::write(r.path().join(&path), bytes).unwrap();
    let line = format!("line {}", lines_before + 1);
    let (_, v) = r.json(&["check"]);
    let s = v.to_string();
    assert!(s.contains("not valid UTF-8") && s.contains(&line), "{s}");
    assert!(!s.contains("no frontmatter block"), "{s}");
    let (_, out) = r.text(&["fix", "--rule", "S1"]);
    assert!(out.contains("not valid UTF-8") && out.contains(&path), "{out}");
    let (code, v) = r.json(&["show", &id]);
    assert_ne!(code, 0);
    let s = v.to_string();
    assert!(s.contains("not valid UTF-8") && s.contains(&line) && s.contains(&path), "{s}");
}

#[test]
fn set_aliases_points_at_the_duplicate_alias_check() {
    let r = common::repo();
    let a = r.new_doc("adr", "services/shop", "First");
    let (_, v) = r.json(&["set", &a, "aliases=PAY-1"]);
    assert!(v["next"].as_str().unwrap().contains("--rule I2"), "{v}");
}

#[test]
fn dropped_non_ascii_words_warn() {
    let r = common::repo();
    let (code, v) = r.json(&["new", "adr", "--governs", "services/shop", "--title", "결제 비교 Gateway"]);
    assert_eq!(code, 0, "{v}");
    let note = v["slug_note"].as_str().unwrap_or_else(|| panic!("{v}"));
    assert!(note.contains("결제 비교"), "{note}");
    assert!(v["next"].as_str().unwrap().starts_with("spec mv "), "{v}");
    let (_, v) = r.json(&["new", "adr", "--governs", "services/shop", "--title", "Plain Gateway"]);
    assert!(v.get("slug_note").is_none(), "{v}");
}

#[test]
fn errors_name_a_real_next_command() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Pick a gateway");
    let (code, v) = r.json(&["edit", &id, "--section", "Risks", "--text", "none", "--write"]);
    assert_ne!(code, 0);
    assert_eq!(v["next"], format!("spec edit {id} --new-section Risks --text=none --write"), "{v}");
    let (code, v) = r.json(&["graph", "--format", "json"]);
    assert_eq!(code, 2, "{v}");
    assert!(v.to_string().contains("--json"), "{v}");
    assert_eq!(v["next"], "spec graph --json");
}

#[cfg(unix)]
#[test]
fn c215_non_utf8_arguments_are_a_usage_error() {
    use std::os::unix::ffi::OsStrExt;
    let r = common::repo();
    let bad = std::ffi::OsStr::from_bytes(b"services/\xff");
    let out = r.spec().arg("context").arg(bad).arg("--json").output().unwrap();
    assert_eq!(out.status.code(), Some(2), "{}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v["error"].as_str().unwrap().contains("not valid UTF-8"), "{v}");
    assert!(!String::from_utf8_lossy(&out.stderr).contains("panicked"));
    let out = r.spec().arg("context").arg(bad).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stdout).contains("next: spec help"));
}

#[test]
fn c199_writes_keep_a_leading_bom() {
    let r = common::repo();
    let id = r.new_doc("guide", "services/shop", "Bom");
    let (_, v) = r.json(&["show", &id]);
    let path = v["path"].as_str().unwrap().to_string();
    r.write(&path, &format!("\u{FEFF}{}", r.read(&path).replace('\n', "\r\n")));
    r.commit("bom");
    let (code, v) = r.json(&["set", &id, "title=X"]);
    assert_eq!(code, 0, "{v}");
    assert!(!v.to_string().contains("\\ufeff"), "the preview shows no BOM change: {v}");
    for args in
        [&["set", &id, "title=X", "--write"][..], &["edit", &id, "--section", "Steps", "--text", "Two.", "--write"]]
    {
        let (code, v) = r.json(args);
        assert_eq!(code, 0, "{v}");
        let text = r.read(&path);
        assert!(text.starts_with("\u{FEFF}---\r\n"), "{args:?}: {text:?}");
    }
}

#[test]
fn c200_wrong_type_title_and_status_are_errors() {
    let r = common::repo();
    let id = r.new_doc("guide", "services/shop", "Typed");
    let (_, v) = r.json(&["show", &id]);
    let path = v["path"].as_str().unwrap().to_string();
    let (code, v) = r.json(&["set", &id, "title=[x, y]", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert!(
        r.read(&path).contains("title: '[x, y]'") || r.read(&path).contains("title: \"[x, y]\""),
        "{}",
        r.read(&path)
    );
    let (_, v) = r.json(&["show", &id]);
    assert_eq!(v["frontmatter"]["title"], "[x, y]", "{v}");
    let text = r.read(&path);
    let text: String = text
        .lines()
        .map(|l| {
            if l.starts_with("title:") {
                "title: [x, y]".into()
            } else if l.starts_with("status:") {
                "status: [active]".into()
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    r.write(&path, &text);
    let (code, v) = r.json(&["check"]);
    assert_eq!(code, 1, "{v}");
    let s = v.to_string();
    assert!(s.contains("not a non-empty string") && s.contains("S1"), "{s}");
    assert!(s.contains("not a string") && s.contains("S4"), "{s}");
}
