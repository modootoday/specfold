//! The CLI contract (C1–C10) and the CLI conventions (CLI-1–CLI-16) checked for every command, so an
//! agent can rely on them. Contracts C1–C10 are not the usability findings C1… in research notes.

mod common;

use serde_json::Value;

/// One realistic invocation per command, run against a populated repository.
fn invocations(sot: &str, adr: &str) -> Vec<Vec<String>> {
    let v = |s: &[&str]| s.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    vec![
        v(&["context", "services/shop/src/cart/total.rs"]),
        v(&["find", "cart"]),
        v(&["show", sot]),
        v(&["show", adr, "--tasks"]),
        v(&["resolve", sot]),
        v(&["graph", "impact", sot]),
        v(&["owners"]),
        v(&["assets"]),
        v(&["log", adr]),
        v(&["new", "plan", "--governs", "services/blog", "--title", "Blog launch", "--dry-run"]),
        v(&["set", adr, "tags+=money", "--dry-run"]),
        v(&["edit", adr, "--section", "Decision", "--text", "Use cards."]),
        v(&["mv", sot, "--slug", "basket"]),
        v(&["fix"]),
        v(&["files", adr]),
        v(&["check"]),
        v(&["verify"]),
        v(&["guard"]),
        v(&["audit"]),
        v(&["census"]),
        v(&["explain", "L1"]),
        v(&["schema", "adr"]),
        v(&["help"]),
        v(&["agents"]),
        v(&["hook", "print"]),
        v(&["plugins"]),
    ]
}

fn populated() -> (common::Repo, String, String) {
    let r = common::repo();
    let sot = r.new_doc("sot", "services/shop/src/cart", "Cart totals");
    let adr = r.new_doc("adr", "services/shop/src/cart", "Round half to even");
    r.commit("docs");
    (r, sot, adr)
}

#[test]
fn c1_every_command_and_argument_is_described() {
    let r = common::repo();
    let (code, v) = r.json(&["help"]);
    assert_eq!(code, 0);
    let items = v["items"].as_array().unwrap();
    assert!(items.len() >= 28, "{} commands", items.len());
    for c in items {
        assert!(!c["about"].as_str().unwrap().is_empty(), "{} has no about", c["name"]);
        for a in c["args"].as_array().unwrap() {
            assert!(!a["help"].as_str().unwrap().is_empty(), "{} --{} has no help", c["name"], a["name"]);
        }
        assert_eq!(c["output"], format!("spec.{}.v1", c["name"].as_str().unwrap()));
    }
}

#[test]
fn c2_c3_c9_json_schema_and_exit_codes_for_every_command() {
    let (r, sot, adr) = populated();
    for args in invocations(&sot, &adr) {
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = r.spec().args(&refs).arg("--json").write_stdin("").output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        let v: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("spec {args:?}: not JSON ({e}): {text}"));
        let name = args[0].as_str();
        assert_eq!(v["schema"], format!("spec.{name}.v1"), "spec {args:?}");
        assert_eq!(
            v["exit"].as_i64().unwrap() as i32,
            out.status.code().unwrap(),
            "spec {args:?}: exit field and code differ"
        );
        assert!(v.get("next").is_some(), "spec {args:?}: no next field");
        assert!(out.status.code().unwrap() < 2, "spec {args:?} failed: {v}");
    }
}

#[test]
fn c4_findings_carry_location_and_fix() {
    let (r, sot, _) = populated();
    let path = r.json(&["resolve", &sot]).1["path"].as_str().unwrap().to_string();
    let text = r.read(&path).replace("created: ", "created: 2026/01/0");
    r.write(&path, &text);
    let (code, v) = r.json(&["check"]);
    assert_eq!(code, 1);
    let f = v["items"].as_array().unwrap().iter().find(|f| f["rule"] == "S5").expect("an S5 finding");
    for k in ["rule", "severity", "path", "line", "message"] {
        assert!(f.get(k).is_some(), "finding lacks {k}");
    }
    assert_eq!(f["path"], path);
}

#[test]
fn c5_writes_are_previews_unless_asked() {
    let (r, sot, _) = populated();
    let before = r.json(&["resolve", &sot]).1["path"].as_str().unwrap().to_string();
    let (code, v) = r.json(&["mv", &sot, "--slug", "basket"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["written"], false);
    assert!(r.path().join(&before).is_file());
    let (code, v) = r.json(&["mv", &sot, "--slug", "basket", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert!(!r.path().join(&before).exists());
    let (code, v) = r.json(&["mv", "sot:shop/basket", "--slug", "basket", "--write"]);
    assert_eq!(code, 0, "moving to where it already is is a no-op: {v}");
    assert_eq!(v["items"].as_array().unwrap().len(), 0);
}

#[test]
fn c6_lists_are_bounded() {
    let (r, _, _) = populated();
    for i in 0..4 {
        r.new_doc("plan", "services/blog", &format!("Plan number {i}"));
    }
    let (_, v) = r.json(&["find", "plan", "--limit", "2"]);
    assert_eq!(v["items"].as_array().unwrap().len(), 2);
    assert_eq!(v["total"], 4);
    assert_eq!(v["more"], 2);
    let (_, t) = r.text(&["find", "plan", "--limit", "2"]);
    assert!(t.contains("+2 more not shown: --offset 2"), "{t}");
}

#[test]
fn c7_one_resolver_for_ids_paths_slugs() {
    let (r, sot, _) = populated();
    let path = r.json(&["resolve", &sot]).1["path"].as_str().unwrap().to_string();
    for q in [sot.as_str(), path.as_str(), "cart-totals"] {
        let (code, v) = r.json(&["resolve", q]);
        assert_eq!(code, 0, "{q}: {v}");
        assert_eq!(v["id"], sot);
    }
    let (code, _) = r.json(&["resolve", "no-such-thing"]);
    assert_eq!(code, 2);
}

#[test]
fn c8_current_documents_by_default() {
    let (r, _, adr) = populated();
    let (code, _) = r.json(&["set", &adr, "status=rejected"]);
    assert_eq!(code, 0);
    assert_eq!(r.json(&["find", "round"]).1["total"], 0);
    assert_eq!(r.json(&["find", "round", "--all"]).1["total"], 1);
}

#[test]
fn c10_text_output_ends_with_next() {
    let (r, sot, _) = populated();
    let (_, t) = r.text(&["show", &sot]);
    assert!(t.trim_end().lines().last().unwrap().starts_with("next: "), "{t}");
}

#[test]
fn usage_errors_exit_2_and_suggest_help() {
    let r = common::repo();
    let (code, v) = r.json(&["nonsense"]);
    assert_eq!(code, 2);
    assert_eq!(v["next"], "spec help");
}

#[test]
fn edit_reads_piped_text_without_text_or_file() {
    let (r, _sot, adr) = populated();
    let out = r
        .spec()
        .args(["edit", &adr, "--section", "Decision", "--json"])
        .write_stdin("Piped decision.\n")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v.to_string().contains("--file <saved-input> --write"), "{v}");
    let out = r
        .spec()
        .args(["edit", &adr, "--section", "Decision", "--replace", "--write"])
        .write_stdin("Piped decision.\n")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let path = r.json(&["resolve", &adr]).1["path"].as_str().unwrap().to_string();
    assert!(r.read(&path).contains("Piped decision."));
    let out = r.spec().args(["edit", &adr, "--section", "Decision"]).write_stdin("").output().unwrap();
    assert_ne!(out.status.code(), Some(0), "empty input is refused");
}

#[test]
fn help_lsp_points_at_the_editor_guide_and_lists_dir() {
    let r = common::repo();
    let (code, t) = r.text(&["help", "lsp"]);
    assert_eq!(code, 0, "{t}");
    assert!(t.contains("  --dir "), "{t}");
    assert!(t.trim_end().ends_with("next: https://specfold.build/guides/editing/#in-an-editor"), "{t}");
}

#[test]
fn help_next_is_the_first_example_without_padding_or_comment() {
    let r = common::repo();
    let (code, t) = r.text(&["help", "guard"]);
    assert_eq!(code, 0, "{t}");
    assert!(t.trim_end().ends_with("next: spec guard"), "{t}");
}

// ---- CLI conventions CLI-1–CLI-16 (AGENTS.md). Rules not implemented yet are ignored with a reason.

#[test]
fn cli1_writes_need_write_flag_everywhere() {
    let (r, sot, adr) = populated();
    let path = r.json(&["resolve", &adr]).1["path"].as_str().unwrap().to_string();
    let before = r.read(&path);
    let (code, v) = r.json(&["edit", &adr, "--section", "Decision", "--text", "Use cards."]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["written"], false);
    assert!(v["next"].as_str().unwrap().contains("--write"), "{v}");
    assert_eq!(r.read(&path), before);
    // --dry-run is accepted by every write command and writes nothing.
    for args in [
        vec!["edit", &adr, "--section", "Decision", "--text", "x", "--dry-run"],
        vec!["mv", &sot, "--slug", "basket", "--dry-run"],
        vec!["fix", "--dry-run"],
        vec!["sync", "assets", "--dry-run"],
        vec!["audit", "--dry-run"],
        vec!["set", &adr, "tags+=money", "--dry-run"],
    ] {
        let (code, v) = r.json(&args);
        assert!(code < 2, "spec {args:?}: {v}");
    }
    assert_eq!(r.read(&path), before);
    // --dry-run and --write together are a usage error.
    let out = r.spec().args(["mv", &sot, "--slug", "b", "--dry-run", "--write"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn cli1_new_without_write_warns_on_stderr_until_0_4() {
    let r = common::repo();
    let out =
        r.spec().args(["new", "plan", "--governs", "services/blog", "--title", "Blog", "--json"]).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("from 0.4 it previews unless --write"), "{err}");
    serde_json::from_slice::<Value>(&out.stdout).expect("stdout stays JSON");
    let out = r
        .spec()
        .args(["new", "plan", "--governs", "services/blog", "--title", "Blog two", "--write"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(!String::from_utf8_lossy(&out.stderr).contains("warning"));
}

#[test]
fn cli2_every_command_declares_risk() {
    let r = common::repo();
    let (_, v) = r.json(&["help"]);
    for c in v["items"].as_array().unwrap() {
        let risk = c["risk"].as_str().unwrap_or_default();
        assert!(["read", "write", "destructive", "external"].contains(&risk), "{} risk {risk:?}", c["name"]);
    }
    let risk = |n: &str| v["items"].as_array().unwrap().iter().find(|c| c["name"] == n).unwrap()["risk"].clone();
    assert_eq!(risk("show"), "read");
    assert_eq!(risk("edit"), "write");
    assert_eq!(risk("sync"), "destructive");
    assert_eq!(risk("attach"), "external");
}

#[test]
fn cli3_destructive_needs_yes_off_tty() {
    let (r, _, _) = populated();
    r.write("services/shop/src/cart/total.rs", "// TODO: round\n");
    r.commit("code");
    let (_, v) = r.json(&["check"]);
    if v["items"].as_array().unwrap().iter().any(|f| f["severity"] == "error") {
        let (code, v) = r.json(&["check", "--write-baseline"]);
        assert!(code < 2, "{v}");
        assert_eq!(v["written"], false, "{v}");
        assert!(v["next"].as_str().unwrap().ends_with("--yes"), "{v}");
        assert!(!r.path().join(".specfold/baseline.json").exists());
    }
    let (code, v) = r.json(&["guard", "--write-baseline"]);
    assert!(code < 2, "{v}");
    assert_eq!(v["written"], false, "{v}");
    assert!(v["next"].as_str().unwrap().contains("--write-baseline --yes"), "{v}");
    assert!(v["baselined"].is_u64(), "the preview counts what it would record: {v}");
}

#[test]
fn cli4_overwrite_needs_ownership_or_force() {
    let r = common::repo();
    let base = r.read("spec.toml");
    r.write(
        "spec.toml",
        &format!("{base}[[assets]]\nkind = \"skill\"\npath = \".claude/skills\"\ncopy_of = \".agents/skills\"\n"),
    );
    let skill = |d: &str| format!("---\nname: db\ndescription: {d}\n---\n");
    r.write(".agents/skills/db/SKILL.md", &skill("One."));
    r.commit("source");
    let (code, v) = r.json(&["sync", "assets", "--write"]);
    assert_eq!((code, v["written"].clone()), (0, serde_json::json!(true)), "a new copy needs no --force: {v}");
    r.commit("copy");
    // The copy holds a past version of its source: sync's own, overwritten without --force.
    r.write(".agents/skills/db/SKILL.md", &skill("Two."));
    let (_, v) = r.json(&["sync", "assets", "--write"]);
    assert_eq!(v["written"], true, "{v}");
    // Someone edited the copy by hand: no version of the source matches, so --force is needed.
    r.write(".claude/skills/db/SKILL.md", &skill("Hand edit."));
    let (_, v) = r.json(&["sync", "assets", "--write"]);
    assert_eq!(v["written"], false, "{v}");
    assert!(r.read(".claude/skills/db/SKILL.md").contains("Hand edit."));
    let (_, v) = r.json(&["sync", "assets", "--write", "--force"]);
    assert_eq!(v["written"], true, "{v}");
    assert!(r.read(".claude/skills/db/SKILL.md").contains("Two."));
}

#[test]
fn cli5_writes_list_changes_and_an_undo() {
    let (r, _, adr) = populated();
    let (code, v) = r.json(&["edit", &adr, "--section", "Decision", "--text", "Use cards.", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert!(v["undo"].as_str().unwrap().starts_with("git stash push --include-untracked -- "), "{v}");
    let (_, text) = r.text(&["edit", &adr, "--section", "Decision", "--text", "Use cash.", "--write"]);
    assert!(text.contains("undo: git stash push"), "{text}");
}

#[test]
fn cli7_bulk_writes_are_bounded() {
    let (r, sot, _) = populated();
    let toml = r.read("spec.toml");
    r.write("spec.toml", &format!("{toml}[write]\nmax_files = 1\n"));
    r.commit("bound");
    // A move touches two files: over the bound, --write alone writes nothing and says why.
    let (code, v) = r.json(&["mv", &sot, "--slug", "basket", "--write"]);
    assert_eq!(code, 4, "CLI-17: a refused write is not success: {v}");
    assert_eq!(v["written"], false, "{v}");
    assert!(v["next"].to_string().contains("--slug basket"), "C108: the next keeps the flags: {v}");
    assert_eq!(v["files"], 2, "{v}");
    assert_eq!(v["max_files"], 1, "{v}");
    assert!(v["next"].to_string().contains("--yes"), "{v}");
    let (code, text) = r.text(&["mv", &sot, "--slug", "basket"]);
    assert_eq!(code, 0, "a preview without --write is not refused");
    assert!(text.contains("over [write] max_files = 1"), "{text}");
    let (code, v) = r.json(&["mv", &sot, "--slug", "basket", "--write", "--yes"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["written"], true, "{v}");
}

#[test]
fn cli8_stdout_is_data_only() {
    let r = common::repo();
    let out =
        r.spec().args(["new", "plan", "--governs", "services/blog", "--title", "Blog", "--json"]).output().unwrap();
    serde_json::from_slice::<Value>(&out.stdout).expect("diagnostics stay off stdout");
}

#[test]
fn cli9_no_color() {
    let (r, sot, _) = populated();
    for env in [Some("1"), None] {
        let mut c = r.spec();
        c.args(["show", &sot]);
        if let Some(v) = env {
            c.env("NO_COLOR", v);
        }
        let out = c.output().unwrap();
        assert!(!out.stdout.contains(&0x1b), "ESC in piped output (NO_COLOR={env:?})");
        assert!(!out.stderr.contains(&0x1b), "ESC in piped stderr (NO_COLOR={env:?})");
    }
}

/// CLI-11: every long flag used by more than one command, with its one meaning. A flag whose
/// commands disagree is listed in SPLIT with the reason; adding a new shared flag means adding
/// it here, which is the review point.
const SHARED: &[(&str, &str)] = &[
    ("all", "widen the default selection"),
    ("body", "the document body"),
    ("changed", "since this revision"),
    ("dry-run", "preview, write nothing"),
    ("fail-on", "what makes the run exit 1"),
    ("for", "the target tool"),
    ("format", "output format"),
    ("from", "the input to start from"),
    ("kind", "only these kinds"),
    ("local", "the git-ignored local directory"),
    ("owner", "owner"),
    ("rule", "only these rules"),
    ("section", "one section by heading or anchor"),
    ("slug", "the slug"),
    ("staged", "only the index"),
    ("title", "the title"),
    ("translations", "translation pairs"),
    ("type", "only these document types"),
    ("write", "apply, otherwise preview"),
    ("write-baseline", "record current hits as the baseline"),
    ("yes", "confirm a destructive or external action"),
    ("force", "overwrite a file with no sign specfold wrote it"),
];

/// CLI-11 known splits; empty since guard --tree, find --body and init --owner-kind (0.3).
const SPLIT: &[(&str, &str)] = &[];

#[test]
fn cli11_shared_flags_have_one_meaning() {
    let r = common::repo();
    let (_, v) = r.json(&["help"]);
    let mut uses: std::collections::BTreeMap<String, Vec<(String, String)>> = Default::default();
    for c in v["items"].as_array().unwrap() {
        for a in c["args"].as_array().unwrap() {
            if let Some(l) = a["long"].as_str() {
                let entry = (c["name"].as_str().unwrap().to_string(), a["kind"].as_str().unwrap().to_string());
                uses.entry(l.to_string()).or_default().push(entry);
            }
        }
    }
    for (flag, cmds) in uses.iter().filter(|(_, c)| c.len() > 1) {
        assert!(SHARED.iter().any(|(f, _)| f == flag), "--{flag} is shared by {cmds:?}: add its meaning to SHARED");
        let kinds: std::collections::BTreeSet<_> = cmds.iter().map(|(_, k)| k.trim_end_matches("[]")).collect();
        assert!(
            kinds.len() == 1 || SPLIT.iter().any(|(f, _)| f == flag),
            "--{flag} is a switch in one command and takes a value in another: {cmds:?}"
        );
    }
    for (f, _) in SHARED {
        assert!(uses.get(*f).is_some_and(|c| c.len() > 1), "--{f} is no longer shared: drop it from SHARED");
    }
}

#[test]
fn cli12_default_help_is_short() {
    let r = common::repo();
    let (code, short) = r.text(&["--help"]);
    assert_eq!(code, 0, "{short}");
    let listed = |t: &str| t.lines().skip_while(|l| *l != "Commands:").skip(1).take_while(|l| !l.is_empty()).count();
    assert_eq!(listed(&short), 12, "{short}");
    assert!(short.contains("spec --help --all") && !short.contains("  lsp "), "{short}");
    let (_, full) = r.text(&["--help", "--all"]);
    assert!(listed(&full) > 12 && full.contains("  lsp "), "{full}");
    let (_, t) = r.text(&["help"]);
    assert_eq!(t.lines().filter(|l| !l.is_empty() && !l.starts_with(' ')).count(), 12 + 2, "{t}");
    assert!(t.contains("more commands: spec help --all"), "{t}");
    let (_, t) = r.text(&["help", "--all"]);
    assert!(t.contains("lsp ") && !t.contains("more commands"), "{t}");
    let (_, v) = r.json(&["help"]);
    let items = v["items"].as_array().unwrap();
    assert!(items.iter().any(|c| c["name"] == "lsp" && c["core"] == false), "JSON lists every command");
    assert_eq!(items.iter().filter(|c| c["core"] == true).count(), 12);
    let (code, t) = r.text(&["census", "--help"]);
    assert_eq!(code, 0, "a command left out of the short help still runs: {t}");
}

#[test]
fn cli13_deprecated_aliases_are_marked_in_help_json() {
    let r = common::repo();
    let (_, v) = r.json(&["help"]);
    for c in v["items"].as_array().unwrap() {
        let d = c["deprecated_aliases"].as_array().unwrap_or_else(|| panic!("{c}"));
        for a in d {
            assert!(a["name"].is_string() && a["use"] == c["name"] && a["since"].is_string(), "{a}");
        }
    }
}

#[test]
fn cli13_renamed_flags_keep_their_old_name_with_a_warning() {
    let r = common::repo();
    let out = r.spec().args(["find", "blog", "--text"]).output().unwrap();
    assert!(out.status.code().unwrap() < 2, "{}", String::from_utf8_lossy(&out.stderr));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("`spec find --text` is deprecated") && err.contains("use --body"), "{err}");
    let out = r.spec().args(["find", "blog", "--body"]).output().unwrap();
    assert!(String::from_utf8_lossy(&out.stderr).is_empty(), "the new name is quiet");
    let (_, v) = r.json(&["help", "guard"]);
    let tree = v["items"][0]["args"].as_array().unwrap().iter().find(|a| a["long"] == "tree").cloned().unwrap();
    assert_eq!(tree["deprecated_aliases"], serde_json::json!(["all"]), "{tree}");
}

#[test]
fn cli6_the_same_write_twice_reports_no_change() {
    let r = common::repo();
    let base = r.read("spec.toml");
    r.write(
        "spec.toml",
        &format!("{base}[[assets]]\nkind = \"skill\"\npath = \".claude/skills\"\ncopy_of = \".agents/skills\"\n"),
    );
    r.write(".agents/skills/db/SKILL.md", "---\nname: db\ndescription: D.\n---\n");
    let (_, v) = r.json(&["sync", "assets", "--write"]);
    assert_eq!(v["written"], true, "{v}");
    let (code, v) = r.json(&["sync", "assets", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["changes"], 0, "a second run has nothing to do: {v}");
}

#[test]
fn cli10_a_flag_beats_project_config() {
    let (r, sot, _) = populated();
    let base = r.read("spec.toml");
    r.write("spec.toml", &format!("{base}[check]\nbaseline = \".specfold/baseline.json\"\n"));
    let (_, v) = r.json(&["resolve", &sot]);
    let path =
        v["path"].as_str().map(String::from).unwrap_or_else(|| v["items"][0]["path"].as_str().unwrap().to_string());
    let text = r.read(&path);
    r.write(&path, &format!("{text}\nSee [[sot:nowhere/missing]].\n"));
    r.commit("broken link");
    let (_, v) = r.json(&["check", "--write-baseline", "--yes"]);
    let recorded = v["baselined"].as_u64().unwrap_or(0);
    assert!(recorded > 0, "the broken link is an error to hold: {v}");
    let (_, v) = r.json(&["check"]);
    assert_eq!(v["errors"], 0, "config: the baseline holds today's errors: {v}");
    let (_, v) = r.json(&["check", "--no-baseline"]);
    assert_eq!(v["errors"].as_u64().unwrap(), recorded, "the flag overrides [check] baseline: {v}");
}

#[test]
fn cli14_skipping_a_safeguard_says_so() {
    let (r, _, _) = populated();
    let out = tempfile::tempdir().unwrap();
    let dir = out.path().join("e");
    let (_, t) = r.text(&["export", "markdown", "--target", "obsidian", "--out", dir.to_str().unwrap(), "--no-guard"]);
    assert!(t.contains("skipped (--no-guard)"), "{t}");
}

#[test]
fn cli15_off_a_terminal_nothing_waits_for_an_answer() {
    let (r, _, _) = populated();
    for args in
        [&["sync", "assets", "--prune", "--write"][..], &["check", "--write-baseline"], &["guard", "--write-baseline"]]
    {
        let out = assert_cmd::Command::cargo_bin("spec")
            .unwrap()
            .args(args)
            .arg("--json")
            .current_dir(r.path())
            .write_stdin("y\nyes\n")
            .timeout(std::time::Duration::from_secs(20))
            .output()
            .unwrap();
        assert!(out.status.code().is_some(), "{args:?} finished without a prompt");
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_ne!(v["written"], true, "an answer on stdin is never read as --yes: {args:?} {v}");
    }
}

#[test]
fn cli16_the_v1_envelope_keeps_its_fields() {
    let (r, _, _) = populated();
    for args in [&["check"][..], &["find"], &["help"]] {
        let (_, v) = r.json(args);
        for k in ["schema", "ok", "exit", "items", "total"] {
            assert!(v.get(k).is_some(), "{args:?} lost {k}: {v}");
        }
    }
    let (_, v) = r.json(&["check"]);
    for k in ["errors", "warnings", "docs_checked", "docs_total", "rules", "elapsed_ms"] {
        assert!(v.get(k).is_some(), "spec.check.v1 lost {k}: {v}");
    }
}

fn git_status(r: &common::Repo) -> String {
    let out = std::process::Command::new("git").args(["status", "--porcelain"]).current_dir(r.path()).output().unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// SD5: a preview reports the same refusal as the write it previews, on the same files, and the
/// refused write leaves the tree as it was. Keys that only say what happened are left out.
fn same_refusal(r: &common::Repo, preview: &[&str], write: &[&str]) {
    let before = git_status(r);
    let (pc, p) = r.json(preview);
    let (wc, w) = r.json(write);
    assert_eq!(pc, 0, "a preview is not refused: {p}");
    assert_eq!(wc, 4, "the write is refused: {w}");
    assert_ne!(w["written"], true, "{w}");
    assert_eq!(git_status(r), before, "a refused write changes nothing");
    let facts = |v: &Value| {
        let mut m = v.as_object().cloned().unwrap_or_default();
        for k in ["written", "next", "undo", "exit", "ok"] {
            m.remove(k);
        }
        m
    };
    assert_eq!(facts(&p), facts(&w), "preview {preview:?} and write {write:?} disagree");
}

#[test]
fn sd5_previews_and_writes_refuse_alike() {
    let (r, sot, _) = populated();
    let plan = r.new_doc("plan", "services/shop", "Checkout");
    assert_eq!(r.json(&["set", &plan, "status=active", "--write"]).0, 0);
    let (_, d) = r.json(&["show", &plan]);
    let path = d["path"].as_str().unwrap().to_string();
    r.write(&path, &(r.read(&path) + "\n- [ ] payment\n"));
    let toml = r.read("spec.toml");
    r.write("spec.toml", &format!("{toml}[write]\nmax_files = 1\n"));
    r.commit("bound");
    same_refusal(&r, &["mv", &sot, "--slug", "basket"], &["mv", &sot, "--slug", "basket", "--write"]);
    same_refusal(&r, &["set", &plan, "status=archived", "--dry-run"], &["set", &plan, "status=archived", "--write"]);
}

#[test]
fn sd5_sync_export_and_attach_previews_refuse_alike() {
    let r = common::repo();
    let base = r.read("spec.toml");
    let store = r.path().join("store");
    std::fs::create_dir_all(&store).unwrap();
    r.write(
        "spec.toml",
        &format!(
            "{base}[[assets]]\nkind = \"skill\"\npath = \".agents/skills\"\n[[assets]]\nkind = \"skill\"\npath = \".claude/skills\"\ncopy_of = \".agents/skills\"\n[files.store]\nkind = \"dir\"\npath = \"{}\"\n",
            store.display()
        ),
    );
    r.write(".agents/skills/pdf/SKILL.md", "---\nname: pdf\ndescription: Read PDFs.\n---\n");
    r.write(".claude/skills/pdf/SKILL.md", "---\nname: pdf\ndescription: Old.\n---\n");
    let d = r.new_doc("design", "services/shop", "Runs");
    r.write("run.log", "ok\n");
    r.commit("fixtures");
    // A copy that was never a version of its source (CLI-4).
    same_refusal(&r, &["sync", "assets"], &["sync", "assets", "--write"]);
    r.write(".claude/skills/pdf/SKILL.md", "---\nname: pdf\ndescription: Read PDFs.\n---\n");
    r.write(".claude/skills/stale/SKILL.md", "---\nname: stale\ndescription: S.\n---\n");
    r.commit("stale");
    same_refusal(&r, &["sync", "assets", "--prune"], &["sync", "assets", "--prune", "--write"]);
    same_refusal(&r, &["attach", &d, "run.log", "--store"], &["attach", &d, "run.log", "--store", "--write"]);
    let out = r.path().join("vault");
    let o = out.to_str().unwrap();
    assert_eq!(r.json(&["export", "markdown", "--target", "obsidian", "--out", o]).0, 0);
    same_refusal(
        &r,
        &["export", "markdown", "--target", "obsidian", "--out", o, "--dry-run"],
        &["export", "markdown", "--target", "obsidian", "--out", o],
    );
}
