//! Reading: `spec show`, `spec find`, `spec graph` and the one resolver.

mod common;

#[test]
fn show_counts_task_items_with_file_lines() {
    let r = common::repo();
    let id = r.new_doc("plan", "services/shop", "Checkout");
    let (_, v) = r.json(&["show", &id]);
    assert_eq!(v["tasks"]["total"], 0);
    let path = v["path"].as_str().unwrap().to_string();
    let text = r.read(&path) + "\n- [x] cart\n- [ ] payment\n```\n- [ ] sample\n```\n";
    r.write(&path, &text);
    let (code, v) = r.json(&["show", &id, "--tasks"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["tasks"], serde_json::json!({"done": 1, "open": 1, "total": 2}));
    let open = &v["task_items"][1];
    assert_eq!(open["text"], "payment");
    let line = open["line"].as_u64().unwrap() as usize;
    assert_eq!(text.lines().nth(line - 1), Some("- [ ] payment"));
}

#[test]
fn graph_draws_trees_diagrams_relations_and_chains_both_ways() {
    let r = common::repo();
    let old = r.new_doc("adr", "services/shop", "Use files");
    assert_eq!(r.json(&["set", &old, "status=active"]).0, 0);
    let (_, v) = r.json(&["new", "adr", "--from", &old, "--governs", "services/shop", "--title", "Use a database"]);
    let new = v["id"].as_str().unwrap().to_string();
    let sot = r.new_doc("sot", "services/shop/src", "Cart");
    let plan = r.new_doc("plan", "services/shop", "Ship");
    assert_eq!(r.json(&["set", &plan, &format!("rel.implements={sot}")]).0, 0);

    let (code, text) = r.text(&["graph"]);
    assert_eq!(code, 0, "{text}");
    assert!(text.contains(&format!("├─ supersedes → {old}")), "{text}");
    assert!(text.contains("governs → services/shop"), "{text}");
    let (_, text) = r.text(&["graph", "--ascii"]);
    assert!(text.contains(&format!("|-- supersedes -> {old}")) && !text.contains("├"), "{text}");
    let (_, text) = r.text(&["graph", "--depth", "1"]);
    assert!(!text.contains("    "), "depth 1 shows only direct relations: {text}");

    let (_, text) = r.text(&["graph", "--format", "mermaid"]);
    assert!(
        text.starts_with("graph LR") && text.contains("-->|supersedes|") && text.contains("-.->|governs|"),
        "{text}"
    );
    assert!(!text.contains("next:"), "a diagram is only the diagram: {text}");
    let (_, text) = r.text(&["graph", "--format", "dot"]);
    assert!(
        text.starts_with("digraph specfold {")
            && text.contains(&format!("\"{new}\" -> \"{old}\" [label=\"supersedes\"]")),
        "{text}"
    );
    assert_ne!(r.text(&["graph", "impact", &sot, "--format", "dot"]).0, 0);

    let (_, v) = r.json(&["graph", "impact", &sot]);
    assert_eq!((v["items"][0]["kind"].as_str(), v["items"][0]["depth"].as_u64()), (Some("implements"), Some(1)), "{v}");
    let (_, v) = r.json(&["graph"]);
    assert!(v["governs"].to_string().contains("services/shop/src"), "governs is in --json: {v}");

    let (_, v) = r.json(&["graph", "chain", &new]);
    let ids: Vec<_> = v["items"].as_array().unwrap().iter().map(|i| i["id"].as_str().unwrap().to_string()).collect();
    assert_eq!(ids, vec![old, new], "the chain from the new document shows what it replaced");
    assert_eq!(v["items"][1]["this"], true);
}

#[test]
fn box_graph_fits_columns_and_falls_back_to_ascii_or_the_tree() {
    let r = common::repo();
    let sot = r.new_doc("sot", "services/shop/src", "Cart");
    let plan = r.new_doc("plan", "services/shop", "Ship");
    assert_eq!(r.json(&["set", &plan, &format!("rel.implements={sot}")]).0, 0);
    let out = |cols: &str, locale: &str, extra: &[&str]| {
        let mut c = r.spec();
        c.args(["graph", "--format", "box"]).args(extra).env("COLUMNS", cols).env("LC_ALL", locale);
        String::from_utf8_lossy(&c.output().unwrap().stdout).into_owned()
    };
    let text = out("120", "C.UTF-8", &[]);
    assert!(text.contains("┌") && text.contains("i▶") && text.contains("i implements"), "{text}");
    let widest = text.lines().filter(|l| !l.starts_with("next:")).map(|l| l.chars().count()).max().unwrap();
    assert!(widest <= 120, "{text}");
    let text = out("120", "C", &[]);
    assert!(text.contains("i>") && text.lines().all(|l| l.is_ascii()), "a non-UTF-8 locale draws ASCII: {text}");
    assert!(out("120", "C.UTF-8", &["--ascii"]).contains("+-"));
    let text = out("30", "C.UTF-8", &[]);
    assert!(text.contains("COLUMNS is 30") && text.contains("implements →"), "too narrow shows the tree: {text}");
    let (_, v) = r.json(&["graph", "--format", "box"]);
    assert!(v["diagram"].as_str().unwrap().contains(&sot), "{v}");
}

#[test]
fn next_lines_name_real_rules_and_paths_lead_to_governs() {
    let r = common::repo();
    let (_, v) = r.json(&["explain", "Z1"]);
    assert_eq!(v["next"], "spec check --rule Z1");
    let (code, v) = r.json(&["graph", "impact", "services/shop/src"]);
    assert_ne!(code, 0);
    assert_eq!(v["next"], "spec graph governs services/shop/src", "{v}");
}

#[test]
fn a_missing_target_suggests_a_real_id() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Use files");
    for args in [&["graph", "impact"][..], &["graph", "chain"][..], &["show", "--tasks"][..]] {
        let (code, v) = r.json(args);
        assert_ne!(code, 0, "{args:?}");
        let next = v["next"].as_str().unwrap_or_default();
        assert!(next.contains(&id), "{args:?}: {v}");
    }
    let (_, v) = r.json(&["graph", "tree"]);
    assert_eq!(v["next"], "spec graph", "{v}");
}

#[test]
fn find_open_tasks_lists_unfinished_documents() {
    let r = common::repo();
    let done = r.new_doc("plan", "services/shop", "Done plan");
    let open = r.new_doc("plan", "services/shop", "Open plan");
    for (id, body) in [(&done, "- [x] ship\n"), (&open, "- [ ] ship\n- [ ] test\n- [x] plan\n")] {
        let (_, v) = r.json(&["show", id]);
        let path = r.path().join(v["path"].as_str().unwrap());
        let mut text = std::fs::read_to_string(&path).unwrap();
        text.push_str(&format!("\n{body}"));
        std::fs::write(&path, text).unwrap();
    }
    let (code, v) = r.json(&["find", "--open-tasks"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["total"], 1, "{v}");
    assert_eq!(v["items"][0]["id"], open.as_str(), "{v}");
    assert_eq!(v["items"][0]["open_tasks"], 2, "{v}");
    assert_eq!(v["next"], format!("spec show {open} --tasks"), "{v}");
}

#[test]
fn max_bytes_keeps_whole_items_and_points_at_the_rest() {
    let r = common::repo();
    for t in ["Cart", "Pay", "Ship"] {
        r.new_doc("sot", "services/shop", t);
    }
    let (_, all) = r.json(&["find"]);
    assert_eq!(all["items"].as_array().unwrap().len(), 3);
    assert!(all.get("cut_by").is_none());
    let (code, v) = r.json(&["find", "--max-bytes", "1"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["items"].as_array().unwrap().len(), 1, "always at least one whole item");
    assert_eq!(v["more"], 2);
    assert_eq!(v["cut_by"], "max_bytes");
    assert_eq!(v["items"][0], all["items"][0]);
    let (_, v) = r.json(&["find", "--max-bytes", "1", "--offset", "1"]);
    assert_eq!(v["items"][0], all["items"][1]);
    let input = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"spec_find\",\"arguments\":{\"max_bytes\":1}}}\n";
    let out = r.spec().arg("mcp").write_stdin(input).output().unwrap();
    let line: serde_json::Value =
        serde_json::from_str(String::from_utf8_lossy(&out.stdout).lines().next().unwrap()).unwrap();
    let payload: serde_json::Value =
        serde_json::from_str(line["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(payload["more"], 2, "{payload}");
}

#[test]
fn show_body_splits_at_headings_under_max_bytes() {
    let r = common::repo();
    let id = r.new_doc("plan", "services/shop", "Checkout");
    let (_, v) = r.json(&["show", &id]);
    let path = v["path"].as_str().unwrap().to_string();
    let long = "x".repeat(400);
    r.write(
        &path,
        &(r.read(&path) + &format!("\n## Alpha\n\n{long}\n\n## Beta\n\n```\n# not a heading\n```\n{long}\n")),
    );
    let (_, full) = r.json(&["show", &id, "--body"]);
    assert!(full.get("cut_by").is_none());
    let (code, v) = r.json(&["show", &id, "--body", "--max-bytes", "600"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["cut_by"], "max_bytes", "{v}");
    assert_eq!(v["rest_sections"], serde_json::json!(["Beta"]), "{v}");
    assert!(v["next"].as_str().unwrap().contains("--section 'Beta'"), "{v}");
    assert!(!v["body"].as_str().unwrap().contains("## Beta"));
}

#[test]
fn graph_json_and_id_prefixes_get_real_next_steps() {
    let r = common::repo();
    let id = r.new_doc("plan", "services/shop", "Checkout");
    let (_, text) = r.text(&["graph", "json"]);
    assert!(text.contains("spec graph --json"), "{text}");
    let prefix = &id[..id.len() - 4];
    let (code, text) = r.text(&["show", prefix]);
    assert_ne!(code, 0);
    assert!(text.contains(&id), "{text}");
}

#[test]
fn an_unknown_id_points_at_a_search_not_at_help() {
    let r = common::repo();
    let (code, out) = r.text(&["show", "nope"]);
    assert_ne!(code, 0);
    assert!(out.contains("next: spec find nope"), "{out}");
    assert!(!out.contains("spec help show"), "{out}");
}

#[test]
fn a_unique_timestamp_part_of_an_id_opens_the_document() {
    let r = common::repo();
    let (code, v) = r.json(&["new", "adr", "--title", "Use files"]);
    assert_eq!(code, 0, "{v}");
    let id = v["id"].as_str().unwrap().to_string();
    let stamp = id.rsplit('/').next().unwrap().to_string();
    let (code, d) = r.json(&["show", &stamp]);
    assert_eq!(code, 0, "{d}");
    assert_eq!(d["id"], id.as_str(), "{d}");
    // Part of the timestamp is still only a prefix: it names candidates, it does not open one.
    let (code, text) = r.text(&["show", &stamp[..10]]);
    assert_ne!(code, 0, "{text}");
    assert!(text.contains("starts with it") && text.contains(&id), "{text}");
}

#[test]
fn a_timestamp_keyed_id_resolves_by_its_slug_in_place_of_the_timestamp() {
    let r = common::repo();
    let id = r.new_doc("plan", "services/shop", "Payment rollout");
    let (head, _) = id.rsplit_once('/').unwrap();
    let (code, v) = r.json(&["resolve", &format!("{head}/payment-rollout")]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["id"], id.as_str(), "{v}");
    let (code, _) = r.json(&["resolve", "plan:nowhere/payment-rollout"]);
    assert_eq!(code, 2);
}

#[test]
fn a_slug_shared_by_two_timestamp_keyed_ids_lists_both() {
    let r = common::repo();
    let a = r.new_doc("plan", "services/shop", "Payment rollout");
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let b = r.new_doc("plan", "services/shop", "Payment rollout");
    let (head, _) = a.rsplit_once('/').unwrap();
    let (code, v) = r.json(&["resolve", &format!("{head}/payment-rollout")]);
    assert_ne!(code, 0, "{v}");
    let s = v.to_string();
    assert!(s.contains(&a) && s.contains(&b), "both candidates are named: {s}");
}

#[test]
fn show_names_the_sections_when_one_is_missing() {
    let r = common::repo();
    let id = r.new_doc("plan", "services/shop", "Payment rollout");
    let (code, v) = r.json(&["show", &id, "--section", "Nope"]);
    assert_ne!(code, 0, "{v}");
    let s = v.to_string();
    assert!(s.contains("Nope") && s.contains("sections: "), "{s}");
    assert!(!s.contains("sections: Payment rollout"), "the title is not a section: {s}");
}

#[test]
fn a_slug_id_resolves_in_links_and_find() {
    let r = common::repo();
    let id = r.new_doc("plan", "services/shop", "Payment rollout");
    let (head, _) = id.rsplit_once('/').unwrap();
    let slug_id = format!("{head}/payment-rollout");
    let (code, v) = r.json(&["find", &slug_id]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["next"], format!("spec show {id}"), "{v}");
    let other = r.new_doc("adr", "services/shop", "Payments");
    let path = r.json(&["show", &other]).1["path"].as_str().unwrap().to_string();
    r.write(&path, &(r.read(&path) + &format!("\nSee [[{slug_id}]].\n")));
    let (_, v) = r.json(&["check", "--rule", "L1"]);
    let s = v.to_string();
    assert!(s.contains("by its slug") && !s.contains("names no document"), "{s}");
    assert_eq!(r.json(&["fix", "--rule", "L1", "--write"]).0, 0);
    assert!(r.read(&path).contains(&format!("[[{id}]]")), "rewritten to the canonical id");
    let (_, v) = r.json(&["find", "no-such-words", "--type", "plan"]);
    assert_eq!(v["next"], "spec find --all no-such-words --type plan", "{v}");
}

#[test]
fn c172_context_next_names_a_section_only_for_a_sot() {
    let r = common::repo();
    r.write("services/shop/main.rs", "fn main() {}\n");
    let adr = r.new_doc("adr", "services/shop", "Use files");
    assert_eq!(r.json(&["set", &adr, "status=active", "--write"]).0, 0);
    let (code, v) = r.json(&["context", "services/shop/main.rs"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["next"], format!("spec show {adr}"), "{v}");
    let sot = r.new_doc("sot", "services/shop", "Cart");
    assert_eq!(r.json(&["set", &sot, "status=active", "--write"]).0, 0);
    let (_, v) = r.json(&["context", "services/shop/main.rs"]);
    let next = v["next"].as_str().unwrap().to_string();
    let words: Vec<&str> = next.split(' ').skip(1).collect();
    assert_eq!(r.json(&words).0, 0, "{next}");
}

/// C173: every `--section` a help example names is a section of a new document of that type.
#[test]
fn c173_help_example_sections_exist_in_new_documents() {
    let r = common::repo();
    let mut seen = 0;
    for cmd in ["show", "edit"] {
        let (_, help) = r.text(&[cmd, "--help"]);
        for line in help.lines().map(str::trim).filter(|l| l.starts_with("spec ") && l.contains("--section ")) {
            let words: Vec<&str> = line.split_whitespace().collect();
            let Some(at) = words.iter().position(|w| *w == "--section") else { continue };
            let ty = words.iter().find_map(|w| w.split_once(':').map(|(t, _)| t)).unwrap();
            let id = r.new_doc(ty, "services/shop", &format!("Example {seen}"));
            let (code, v) = r.json(&["show", &id, "--section", words[at + 1]]);
            assert_eq!(code, 0, "{line}: {v}");
            seen += 1;
        }
    }
    assert!(seen >= 3, "{seen}");
}

#[test]
fn c176_context_next_for_an_ungoverned_path_runs_as_printed() {
    let r = common::repo();
    let bin = r.spec().get_program().to_string_lossy().into_owned();
    for (n, path) in ["src/new dir", "src/it's", "src/새 폴더"].into_iter().enumerate() {
        r.write(&format!("{path}/main.rs"), "fn main() {}\n");
        let (_, v) = r.json(&["context", path]);
        let next = v["next"].as_str().unwrap_or_default().to_string();
        assert!(next.starts_with("spec new sot --governs "), "{v}");
        // The shell must hand the path back as one argument, exactly as given.
        let words = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("set -- {}; printf '%s\\n' \"$4\"", next.trim_start_matches("spec ")))
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&words.stdout).trim_end(), path, "{next}");
        let out = std::process::Command::new("sh")
            .arg("-c")
            // `'…'` is the placeholder the reader fills in.
            .arg(format!(
                "{} {}",
                specfold_core::shell::quote(&bin),
                next.trim_start_matches("spec ").replace("'…'", &format!("Totals{n}"))
            ))
            .current_dir(r.path())
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{next}: {}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn c179_body_cut_by_max_bytes_continues_with_the_doc_id() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Use files");
    let (code, v) = r.json(&["--max-bytes", "40", "show", &id, "--body"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["cut_by"], "max_bytes", "the body is long enough to cut: {v}");
    let next = v["next"].as_str().unwrap();
    assert!(next.starts_with(&format!("spec show {id} --section ")), "{v}");
}

#[test]
fn c185_a_path_with_glob_characters_round_trips_from_context_to_new() {
    let r = common::repo();
    r.write("src/[a]/z.rs", "fn z() {}\n");
    r.write("src/a/z.rs", "fn z() {}\n");
    let (_, v) = r.json(&["context", "src/[a]/z.rs"]);
    let next = v["next"].to_string();
    assert!(next.contains("src/[[]a]/z.rs"), "the hint escapes the literal path: {v}");
    let (code, v) = r.json(&["new", "sot", "--governs", "src/[a]/z.rs", "--title", "Zed"]);
    assert_eq!(code, 0, "{v}");
    let id = v["id"].as_str().unwrap().to_string();
    let (_, v) = r.json(&["context", "src/[a]/z.rs"]);
    assert_eq!(v["items"][0]["id"], id.as_str(), "the literal file is governed: {v}");
    let (_, v) = r.json(&["context", "src/a/z.rs"]);
    assert!(!v.to_string().contains(&id), "the class reading must not match: {v}");
}
