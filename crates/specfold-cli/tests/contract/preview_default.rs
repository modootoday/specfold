//! CLI-1: `spec new` and `spec set` preview unless --write, as every write command does.

#[path = "../common/mod.rs"]
mod common;

use serde_json::Value;

/// Words of a `next` command line, with single and double quotes taken off.
fn words(s: &str) -> Vec<String> {
    let (mut out, mut cur, mut quote, mut any, mut esc) = (vec![], String::new(), None, false, false);
    for c in s.chars() {
        if esc {
            cur.push(c);
            esc = false;
            continue;
        }
        match (quote, c) {
            (None | Some('"'), '\\') => esc = true,
            (None, '\'' | '"') => (quote, any) = (Some(c), true),
            (Some(q), c) if c == q => quote = None,
            (None, ' ') => {
                if any || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                any = false;
            }
            _ => cur.push(c),
        }
    }
    if any || !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Runs a `next` command as given (without its `spec`) with --json.
fn follow(r: &common::Repo, next: &str) -> (i32, Value) {
    let w = words(next.strip_prefix("spec ").unwrap_or(next));
    r.json(&w.iter().map(String::as_str).collect::<Vec<_>>())
}

#[test]
fn new_previews_without_write_and_its_next_writes_the_same_file() {
    let r = common::repo();
    for extra in [vec![], vec!["--dry-run"]] {
        let mut a = vec!["new", "sot", "--governs", "services/blog", "--title", "Blog's rules"];
        a.extend(extra.iter());
        let out = r.spec().args(&a).arg("--json").output().unwrap();
        assert_eq!(out.status.code(), Some(0));
        assert!(out.stderr.is_empty(), "no 0.3 warning: {}", String::from_utf8_lossy(&out.stderr));
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["written"], false, "{v}");
        assert!(!r.path().join(v["path"].as_str().unwrap()).exists(), "{v}");
        let next = v["next"].as_str().unwrap();
        assert!(next.starts_with("spec new sot ") && next.ends_with(" --write"), "{v}");
    }
    let (_, preview) = r.json(&["new", "sot", "--governs", "services/blog", "--title", "Blog's rules"]);
    let (code, w) = follow(&r, preview["next"].as_str().unwrap());
    assert_eq!(code, 0, "{w}");
    assert_eq!(w["written"], true, "{w}");
    assert_eq!((&w["id"], &w["path"]), (&preview["id"], &preview["path"]), "preview and write agree");
    assert!(r.read(w["path"].as_str().unwrap()).contains("Blog's rules"));
    let (text_code, text) = r.text(&["new", "adr", "--title", "Plain text"]);
    assert_eq!(text_code, 0);
    assert!(text.contains("would create") && text.contains("preview only; nothing was written"), "{text}");
    assert!(text.contains("next: spec new adr --title 'Plain text' --write"), "{text}");
}

#[test]
fn new_asset_previews_without_write() {
    let r = common::repo();
    let (code, v) = r.json(&["new", "skill", "--title", "review", "--description", "Review a change"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["written"], false, "{v}");
    assert!(!r.path().join(v["path"].as_str().unwrap()).exists());
    let (code, w) = follow(&r, v["next"].as_str().unwrap());
    assert_eq!(code, 0, "{w}");
    assert_eq!(w["written"], true, "{w}");
    assert_eq!(r.read(w["path"].as_str().unwrap()), v["text"].as_str().unwrap(), "preview text is what is written");
}

#[test]
fn set_previews_without_write_and_the_write_matches_the_diff() {
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart rules");
    let path = r.json(&["resolve", &id]).1["path"].as_str().unwrap().to_string();
    let before = r.read(&path);
    let mut previews = vec![];
    for extra in [vec![], vec!["--dry-run"]] {
        let mut a = vec!["set", id.as_str(), "tags+=money"];
        a.extend(extra.iter());
        let out = r.spec().args(&a).arg("--json").output().unwrap();
        assert_eq!(out.status.code(), Some(0));
        assert!(out.stderr.is_empty(), "no 0.3 warning: {}", String::from_utf8_lossy(&out.stderr));
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["written"], false, "{v}");
        assert_eq!(v["next"], format!("spec set {id} tags+=money --write"), "{v}");
        assert_eq!(r.read(&path), before, "a preview writes nothing");
        previews.push(v);
    }
    assert_eq!(previews[0]["items"], previews[1]["items"], "--dry-run is the default preview");
    let (code, w) = follow(&r, previews[0]["next"].as_str().unwrap());
    assert_eq!(code, 0, "{w}");
    assert_eq!(w["written"], true, "{w}");
    let after = r.read(&path);
    let hunks = previews[0]["items"][0]["hunks"].as_array().unwrap();
    assert!(!hunks.is_empty(), "{}", previews[0]);
    for h in hunks {
        for l in h["after"].as_array().unwrap() {
            let l = l.as_str().unwrap();
            assert!(after.lines().any(|x| x == l), "{l:?} previewed but not written: {after}");
        }
    }
    let (code, again) = r.json(&["set", &id, "tags+=money", "--write"]);
    assert_eq!((code, &again["changed"]), (0, &Value::Bool(false)), "{again}");
}

#[test]
fn set_archive_of_a_plan_with_open_tasks_previews_and_refuses_only_the_write() {
    let r = common::repo();
    let id = r.new_doc("plan", "services/shop", "Checkout");
    let path = r.json(&["resolve", &id]).1["path"].as_str().unwrap().to_string();
    let text = r.read(&path);
    r.write(&path, &format!("{text}\n- [ ] ship it\n"));
    let (code, v) = r.json(&["set", &id, "status=archived"]);
    assert_eq!(code, 0, "a preview reports the open tasks: {v}");
    assert!(!v["open_tasks"].as_array().unwrap().is_empty(), "{v}");
    let (code, v) = r.json(&["set", &id, "status=archived", "--write"]);
    assert_eq!(code, 4, "{v}");
    assert!(v["next"].as_str().unwrap().contains("--write --force"), "{v}");
}

#[test]
fn b2_superseding_a_draft_hint_commands_run_as_given() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Use files");
    let newer = r.new_doc("adr", "services/shop", "Use a database");
    let (code, v) = r.json(&["set", &id, "status=superseded", "--write"]);
    assert_ne!(code, 0, "nothing names a successor yet: {v}");
    let msg = v["error"].as_str().unwrap_or_default().to_string();
    let cmds: Vec<String> = msg.split('`').skip(1).step_by(2).map(|c| c.replace("<newer-id>", &newer)).collect();
    assert_eq!(cmds.len(), 1, "{msg}");
    let (code, w) = follow(&r, &cmds[0]);
    assert_eq!((code, &w["written"]), (0, &Value::Bool(true)), "{w}");
    let (code, w) = r.json(&["set", &id, "status=superseded", "--write"]);
    assert_eq!(code, 0, "a draft a newer document replaced is superseded: {w}");
    assert_eq!(r.json(&["check", "--rule", "T1"]).1["errors"], 0);
    let other = r.new_doc("adr", "services/shop", "Use queues");
    let (_, v) = r.json(&["set", &other, "status=superseded", "--write"]);
    let next = v["next"].as_str().unwrap_or_default().replace('…', "Filled in");
    let (code, w) = follow(&r, &next);
    assert_eq!((code, &w["written"]), (0, &Value::Bool(true)), "spec new --from a draft: {w}");
    assert_eq!(r.json(&["show", &other]).1["frontmatter"]["status"], "superseded");
}

/// C246: a superseded document gets no second successor, and its refusals point at the current one.
#[test]
fn a_superseded_document_gets_one_successor_and_points_to_it() {
    let r = common::repo();
    let old = r.new_doc("adr", "services/shop", "Pay");
    assert_eq!(r.json(&["set", &old, "status=active", "--write"]).0, 0);
    let (code, v) = r.json(&["new", "adr", "--title", "Pay two", "--from", &old, "--write"]);
    assert_eq!(code, 0, "{v}");
    let succ = v["id"].as_str().unwrap().to_string();
    let (code, v) = r.json(&["edit", &old, "--text", "x"]);
    assert_eq!(code, 2, "{v}");
    let next = v["next"].as_str().unwrap_or_default().to_string();
    assert!(
        next.starts_with(&format!("spec edit {succ} ")) && next.ends_with(" --write"),
        "a draft successor is edited: {v}"
    );
    for write in [false, true] {
        let mut a = vec!["new", "adr", "--title", "Pay three", "--from", old.as_str()];
        if write {
            a.push("--write");
        }
        let before = r.json(&["find", "--type", "adr"]).1.to_string();
        let (code, v) = r.json(&a);
        assert_eq!(code, 2, "a second successor is refused, preview too: {v}");
        assert!(v["error"].as_str().unwrap_or_default().contains(&succ), "{v}");
        assert_eq!(r.json(&["find", "--type", "adr"]).1.to_string(), before, "nothing written");
    }
    let other = r.new_doc("adr", "services/shop", "Pay other");
    let (code, v) = r.json(&["set", &other, &format!("rel.supersedes+={old}"), "--write"]);
    assert_eq!(code, 2, "set refuses a second successor too: {v}");
    let (code, w) = follow(&r, &next);
    assert_eq!((code, &w["written"]), (0, &Value::Bool(true)), "the hint supersedes the current one: {w}");
    let (code, c) = r.json(&["check"]);
    assert_eq!(code, 0, "no T1: {c}");
    assert_eq!(r.json(&["set", &succ, "status=active", "--write"]).0, 0);
    let (_, v) = r.json(&["edit", &old, "--text", "x"]);
    let next = v["next"].as_str().unwrap_or_default().to_string();
    assert!(next.contains(&format!("--from {succ}")) && next.ends_with(" --write"), "an active one is superseded: {v}");
    assert!(v["error"].as_str().unwrap_or_default().contains(&format!("spec edit {succ}")), "{v}");
    let (code, w) = follow(&r, &next);
    assert_eq!((code, &w["written"]), (0, &Value::Bool(true)), "{w}");
    let (code, c) = r.json(&["check"]);
    assert_eq!(code, 0, "one successor each: {c}");
}

/// E1: a preview stays exit 0, but its last text line says NOT WRITTEN and JSON says preview.
#[test]
fn previews_end_with_not_written_and_writes_do_not() {
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart rules");
    for a in
        [vec!["new", "sot", "--governs", "services/blog", "--title", "Blog"], vec!["set", id.as_str(), "tags+=money"]]
    {
        let out = r.spec().args(&a).output().unwrap();
        assert_eq!(out.status.code(), Some(0));
        let t = String::from_utf8_lossy(&out.stdout);
        assert!(t.trim_end().lines().last().unwrap_or("").starts_with("NOT WRITTEN"), "{t}");
        let (code, v) = r.json(&a);
        assert_eq!(code, 0);
        assert_eq!((&v["written"], &v["preview"]), (&Value::Bool(false), &Value::Bool(true)), "{v}");
    }
    let out = r.spec().args(["set", id.as_str(), "tags+=money", "--write"]).output().unwrap();
    let t = String::from_utf8_lossy(&out.stdout);
    assert!(!t.contains("NOT WRITTEN"), "{t}");
}
