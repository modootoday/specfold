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

fn madr_repo(sources: bool) -> (common::Repo, String) {
    let r = common::repo();
    if sources {
        r.write(
            "docs/adr/0001-use-files.md",
            "# Use files\n\n* Status: accepted\n* Date: 2026-03-01\n\n## Context\n\nWhy.\n",
        );
    }
    let (_, map) = r.text(&["import", "--preset", "madr"]);
    r.write("import.toml", &map);
    r.commit("legacy");
    let status = |r: &common::Repo| {
        let o =
            std::process::Command::new("git").args(["status", "--porcelain"]).current_dir(r.path()).output().unwrap();
        String::from_utf8_lossy(&o.stdout).to_string()
    };
    let s = status(&r);
    (r, s)
}

#[test]
fn c120_import_dry_run_previews_even_with_apply() {
    let (r, before) = madr_repo(true);
    for args in [
        &["import", "--map", "import.toml", "--apply", "--dry-run"][..],
        &["import", "--map", "import.toml", "--dry-run"],
    ] {
        let (code, v) = r.json(args);
        assert_eq!(code, 0, "{args:?}: {v}");
    }
    let o = std::process::Command::new("git").args(["status", "--porcelain"]).current_dir(r.path()).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&o.stdout), before, "nothing written");
    assert_eq!(r.json(&["import", "--map", "import.toml", "--apply", "--dry-run", "--yes"]).0, 2);
    let (_, help) = r.text(&["help", "import"]);
    assert!(help.contains("--dry-run"), "{help}");
}

#[test]
fn c121_import_write_lists_files_and_an_undo_that_works() {
    let (r, before) = madr_repo(true);
    let (code, v) = r.json(&["import", "--map", "import.toml", "--apply", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["written"], true, "{v}");
    let changes = v["changes"].to_string();
    assert!(changes.contains(".spec/_aliases.tsv") && changes.contains("adr"), "{v}");
    let undo = v["undo"].as_str().unwrap().to_string();
    assert!(undo.starts_with("git stash push"), "{undo}");
    let ok = std::process::Command::new("sh").args(["-c", &undo]).current_dir(r.path()).status().unwrap();
    assert!(ok.success(), "{undo}");
    let o = std::process::Command::new("git").args(["status", "--porcelain"]).current_dir(r.path()).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&o.stdout), before, "undo restores the tree");
    // No sources: nothing written, no alias table.
    let (r, _) = madr_repo(false);
    let (code, v) = r.json(&["import", "--map", "import.toml", "--apply", "--write"]);
    assert_eq!((code, v["written"].clone()), (0, serde_json::json!(false)), "{v}");
    assert!(!r.path().join(".spec/_aliases.tsv").exists());
    assert!(!v.to_string().contains("SPECFOLD_IMPORT"), "{v}");
}

#[test]
fn c122_mv_undo_restores_committed_and_uncommitted_documents() {
    for commit in [true, false] {
        let r = common::repo();
        let id = r.new_doc("adr", "services/shop", "Use files");
        if commit {
            r.commit("doc");
        }
        let (_, d) = r.json(&["show", &id]);
        let path = d["path"].as_str().unwrap().to_string();
        let text = r.read(&path);
        // A second later the move bumps `updated`, so the undo must bring the old text back.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let (code, v) = r.json(&["mv", &id, "--slug", "use-plain-files", "--write"]);
        assert_eq!(code, 0, "{v}");
        let undo = v["undo"].as_str().unwrap().to_string();
        let out = std::process::Command::new("sh").args(["-c", &undo]).current_dir(r.path()).output().unwrap();
        assert!(out.status.success(), "commit={commit} {undo}: {}", String::from_utf8_lossy(&out.stderr));
        assert_eq!(r.read(&path), text, "commit={commit}: {undo}");
        let (_, d) = r.json(&["show", &id]);
        assert_eq!(d["path"], path.as_str(), "commit={commit}");
    }
}

#[test]
fn c123_import_undo_restores_committed_and_uncommitted_sources() {
    let (r, _) = madr_repo(true);
    let committed = r.read("docs/adr/0001-use-files.md");
    let loose = "# Untracked\n\n* Status: accepted\n* Date: 2026-03-02\n\n## Context\n\nNot committed.\n";
    r.write("docs/adr/0003-untracked.md", loose);
    let (code, v) = r.json(&["import", "--map", "import.toml", "--apply", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert!(!r.path().join("docs/adr/0003-untracked.md").exists(), "{v}");
    let undo = v["undo"].as_str().unwrap().to_string();
    let out = std::process::Command::new("sh").args(["-c", &undo]).current_dir(r.path()).output().unwrap();
    assert!(out.status.success(), "{undo}: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(r.read("docs/adr/0001-use-files.md"), committed, "{undo}");
    assert_eq!(r.read("docs/adr/0003-untracked.md"), loose, "{undo}");
    assert!(
        !r.path().join(".spec/adr").exists() || std::fs::read_dir(r.path().join(".spec/adr")).unwrap().next().is_none(),
        "{undo}"
    );
    // Text and JSON name the same undo.
    let (r2, _) = madr_repo(true);
    r2.write("docs/adr/0003-untracked.md", loose);
    let (_, t) = r2.text(&["import", "--map", "import.toml", "--apply", "--write"]);
    let line = t.lines().find_map(|l| l.trim().strip_prefix("undo: ")).unwrap_or_default();
    assert!(line.contains(".specfold/trash/") && line.contains("git stash push"), "{t}");
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

#[test]
fn c135_nested_and_tilde_fences_hide_their_contents() {
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Fences");
    let (_, d) = r.json(&["show", &id]);
    let path = d["path"].as_str().unwrap().to_string();
    let text = r.read(&path);
    r.write(
        &path,
        &format!(
            "{text}\n````md\n```\n[[sot:root/nope-one]]\n```\n[[sot:root/nope-two]]\n````\n\n~~~\n```\n[[sot:root/nope-three]]\n~~~\n"
        ),
    );
    let (_, v) = r.json(&["check", "--rule", "L1"]);
    assert!(!v.to_string().contains("nope-"), "{v}");
    // A link after the fence closes is read again.
    r.write(&path, &format!("{text}\n````\n```\n````\n[[sot:root/nope-after]]\n"));
    let (_, v) = r.json(&["check", "--rule", "L1"]);
    assert!(v.to_string().contains("nope-after"), "{v}");
}

#[test]
fn c138_import_bound_counts_the_alias_table() {
    let (r, _) = madr_repo(true);
    let base = r.read("spec.toml");
    // One move (two paths) plus the new alias table is three files.
    r.write("spec.toml", &format!("{base}[write]\nmax_files = 2\n"));
    let (_, v) = r.json(&["import", "--map", "import.toml", "--apply"]);
    assert_eq!(v["files"], 3, "{v}");
    assert_eq!(v["bounded"], true, "{v}");
    r.write("spec.toml", &format!("{base}[write]\nmax_files = 3\n"));
    let (_, v) = r.json(&["import", "--map", "import.toml", "--apply"]);
    assert!(v.get("bounded").is_none(), "{v}");
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

#[test]
fn c124_sync_assets_kind_conflicts_match_and_prune_resolves() {
    let r = common::repo();
    let base = r.read("spec.toml");
    r.write(
        "spec.toml",
        &format!(
            "{base}[[assets]]\nkind = \"skill\"\npath = \".agents/skills\"\n[[assets]]\nkind = \"skill\"\npath = \".claude/skills\"\ncopy_of = \".agents/skills\"\n"
        ),
    );
    r.write(".agents/skills/pdf/SKILL.md", "---\nname: pdf\ndescription: Read PDFs.\n---\n");
    r.write(".agents/skills/pdf/notes", "file in source\n");
    r.write(".claude/skills/pdf/SKILL.md", "---\nname: pdf\ndescription: Read PDFs.\n---\n");
    r.write(".claude/skills/pdf/notes/old.md", "a folder in the copy\n");
    r.commit("fixtures");
    let (pc, pv) = r.json(&["sync", "assets"]);
    let (wc, wv) = r.json(&["sync", "assets", "--write"]);
    assert_eq!(pc, 0, "{pv}");
    assert_eq!(wc, 4, "{wv}");
    assert!(pv.to_string().contains(".claude/skills/pdf/notes"), "{pv}");
    assert!(wv.to_string().contains(".claude/skills/pdf/notes"), "{wv}");
    assert!(pv["next"].to_string().contains("--prune"), "{pv}");
    let (code, v) = r.json(&["sync", "assets", "--prune", "--write", "--yes"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(r.read(".claude/skills/pdf/notes"), "file in source\n");
}

#[test]
fn c147_asset_without_a_slug_is_refused() {
    let r = common::repo();
    for kind in ["skill", "rule", "agent"] {
        let (code, v) = r.json(&["new", kind, "--title", "한글 이름", "--write"]);
        assert_eq!(code, 2, "{kind}: {v}");
        assert!(v.to_string().contains("--slug"), "{v}");
    }
    let (code, v) = r.json(&["new", "skill", "--title", "x", "--slug", "../up", "--write"]);
    assert_eq!(code, 2, "{v}");
}

#[test]
fn c140_init_checks_owner_kinds_and_root() {
    let dir = tempfile_dir();
    for args in [
        &["--owner-kind", "=x"][..],
        &["--owner-kind", "a/*="],
        &["--owner-kind", "services/*={2}"],
        &["--owner-kind", "libs/*=lib.{1}:bogus"],
        &["--root", "Bad Id!"],
    ] {
        let mut full = vec!["init", "--write"];
        full.extend_from_slice(args);
        let o = std::process::Command::new(common::bin_dir().join("spec"))
            .args(&full)
            .arg("--json")
            .current_dir(&dir)
            .output()
            .unwrap();
        assert_eq!(o.status.code(), Some(2), "{args:?}: {}", String::from_utf8_lossy(&o.stdout));
        assert!(!dir.join("spec.toml").exists(), "{args:?}");
    }
    let o = std::process::Command::new(common::bin_dir().join("spec"))
        .args(["init", "--owner-kind", "services/*={1}", "--owner-kind", "libs/*=lib.{1}:manifest", "--root", "ws"])
        .current_dir(&dir)
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
}

fn tempfile_dir() -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("specfold-c140-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::process::Command::new("git").arg("init").arg("-q").current_dir(&d).status().unwrap();
    d
}

#[test]
fn c128_set_refuses_emptying_the_title() {
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart");
    let (code, v) = r.json(&["set", &id, "title=", "--write"]);
    assert_eq!(code, 2, "{v}");
    let (_, d) = r.json(&["show", &id]);
    assert_eq!(d["frontmatter"]["title"], "Cart", "{d}");
    assert_eq!(r.json(&["set", &id, "title=Basket", "--write"]).0, 0);
}

#[test]
fn c155_import_write_refuses_rows_that_need_a_decision() {
    let r = common::repo();
    r.write("docs/adr/0001-use-files.md", "# Use files\n\n* Status: accepted\n\n## Context\n\nWhy.\n");
    let (_, map) = r.text(&["import", "--preset", "madr"]);
    r.write("import.toml", &map);
    r.commit("legacy");
    let (code, v) = r.json(&["import", "--map", "import.toml", "--apply", "--write"]);
    assert_eq!(code, 4, "{v}");
    assert!(r.path().join("docs/adr/0001-use-files.md").exists(), "nothing written: {v}");
    assert!(v["next"].to_string().contains("--force"), "{v}");
    assert!(v.to_string().contains("0001-use-files"), "{v}");
    let (code, v) = r.json(&["import", "--map", "import.toml", "--apply", "--write", "--force"]);
    assert_eq!(code, 0, "{v}");
    assert!(!r.path().join("docs/adr/0001-use-files.md").exists(), "{v}");
}

fn doc_path(r: &common::Repo, id: &str) -> String {
    let (_, d) = r.json(&["show", id]);
    d["path"].as_str().unwrap().to_string()
}

#[test]
fn c154_writes_keep_crlf_line_endings() {
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart");
    let path = doc_path(&r, &id);
    let crlf = r.read(&path).replace('\n', "\r\n");
    r.write(&path, &crlf);
    r.commit("crlf");
    assert_eq!(r.json(&["set", &id, "tags+=x", "--write"]).0, 0);
    assert_eq!(r.json(&["edit", &id, "--section", "Rule", "--text", "New rule.", "--write"]).0, 0);
    let text = r.read(&path);
    assert!(text.contains("New rule.") && text.contains("x"), "{text}");
    assert_eq!(text.matches('\n').count(), text.matches("\r\n").count(), "only CRLF: {text:?}");
}

#[test]
fn c139_an_id_held_by_two_files_is_refused() {
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart");
    let path = doc_path(&r, &id);
    let owner = id.split(':').nth(1).unwrap().split('/').next().unwrap();
    let local = format!(".spec.local/{owner}/sot/cart.sot.md");
    r.write(&local, &r.read(&path));
    r.write(".gitignore", ".spec.local/\n");
    r.commit("dup");
    let (code, v) = r.json(&["mv", &id, "--slug", "cart-local", "--write"]);
    assert_eq!(code, 2, "{v}");
    assert!(v.to_string().contains(&local), "{v}");
    assert!(r.path().join(&path).exists());
    let (_, v) = r.json(&["doctor"]);
    assert!(v.to_string().contains(&format!("spec mv {local}")), "{v}");
}

#[test]
fn c137_mv_rewrites_relative_markdown_links() {
    let r = common::repo();
    let a = r.new_doc("sot", "services/shop", "Cart");
    let b = r.new_doc("sot", "services/shop", "Pay");
    let pb = doc_path(&r, &b);
    let text = r.read(&pb);
    r.write(
        &pb,
        &format!(
            "{text}\nSee [cart](cart.sot.md) and [v](./cart.sot.md#verification) and [w](https://x.io/cart.sot.md).\n"
        ),
    );
    r.commit("links");
    let (code, v) = r.json(&["mv", &a, "--slug", "basket", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert!(v["next"].to_string().contains("L8"), "{v}");
    let t = r.read(&pb);
    assert!(t.contains("[cart](basket.sot.md)") && t.contains("[v](basket.sot.md#verification)"), "{t}");
    assert!(t.contains("https://x.io/cart.sot.md"), "{t}");
    let (_, v) = r.json(&["check", "--rule", "L8"]);
    assert!(!v.to_string().contains("cart.sot.md"), "{v}");
}

#[test]
fn c136_audit_record_passes_check_and_is_no_orphan() {
    let r = common::repo();
    for t in ["Loose one", "Loose two"] {
        let id = r.new_doc("sot", "services/shop", t);
        let p = doc_path(&r, &id);
        let text = r.read(&p).replace("governs:\n  - services/shop\n", "").replace("governs: [services/shop]\n", "");
        r.write(&p, &text);
        r.json(&["set", &id, "rel.governs-=services/shop", "status=active", "--write"]);
    }
    r.commit("loose");
    let (code, v) = r.json(&["audit", "--record", "--title", "four", "--write"]);
    assert_eq!(code, 0, "{v}");
    r.commit("record");
    let (_, v) = r.json(&["check", "--rule", "L2"]);
    assert_eq!(v["errors"], 0, "{v}");
    let (_, v) = r.json(&["list", "--type", "audit"]);
    let rec = v["items"][0]["path"].as_str().unwrap_or_default().to_string();
    assert!(r.read(&rec).contains("[[sot:"), "{v}");
    let (_, v) = r.json(&["audit"]);
    assert!(!v.to_string().contains("\"subject\":\"audit:"), "{v}");
}

#[test]
fn c125_section_text_may_not_start_another_section() {
    let r = common::repo();
    let id = r.new_doc("sot", "services/shop", "Cart");
    for args in [
        &["edit", &id, "--section", "Rule", "--text", "x\n## Why\ndup", "--write"][..],
        &["edit", &id, "--new-section", "Risks", "--text", "- r\n# Top\n", "--write"],
    ] {
        let (code, v) = r.json(args);
        assert_eq!(code, 2, "{args:?}: {v}");
    }
    assert_eq!(
        r.json(&["edit", &id, "--section", "Rule", "--text", "x\n### Detail\n```\n## not a heading\n```", "--write"]).0,
        0
    );
}
