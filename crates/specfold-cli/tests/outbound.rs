//! Content leaving the repository: the guard before `spec attach` uploads to the store, and
//! redaction of secrets in what `spec mcp` hands an agent (S2, S4).

mod common;

fn leaky_url() -> String {
    format!("postgres:{}app:{}{}@db.acme.io/main", "//", "s3cr", "Et-77x")
}

fn with_store(r: &common::Repo, guard: &str) {
    let store = r.path().join("store");
    std::fs::create_dir_all(&store).unwrap();
    let mut toml = r.read("spec.toml");
    toml.push_str(&format!("[files.store]\nkind = \"dir\"\npath = \"{}\"\n{guard}", store.display()));
    r.write("spec.toml", &toml);
}

const SECRETS: &str = "[[guards]]\nname = \"secrets\"\nsecrets = true\nrange = \"all\"\n";

#[test]
fn an_upload_with_a_secret_is_refused_unless_its_guard_is_allowed() {
    let r = common::repo();
    with_store(&r, SECRETS);
    let d = r.new_doc("design", "services/shop", "Runs");
    r.write("run.log", &format!("connecting to {}\n", leaky_url()));
    let (code, v) = r.json(&["attach", &d, "run.log", "--store", "--write", "--yes"]);
    assert_eq!(code, 1, "{v}");
    let s = v.to_string();
    assert!(s.contains("connection string") && !s.contains("Et-77x"), "{s}");
    assert_eq!(v["next"], format!("spec attach {d} run.log --store --write --yes --allow-hit secrets"));
    assert!(r.json(&["files", &d]).1.to_string().find("run.log").is_none(), "nothing attached");
    let (code, v) = r.json(&["attach", &d, "run.log", "--store", "--write", "--yes", "--allow-hit", "secrets"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["guard"][0]["allowed"], true);
}

#[test]
fn a_warn_guard_shows_its_hit_and_uploads() {
    let r = common::repo();
    with_store(&r, &format!("{SECRETS}level = \"warn\"\n"));
    let d = r.new_doc("design", "services/shop", "Runs");
    r.write("run.log", &format!("connecting to {}\n", leaky_url()));
    let (code, v) = r.json(&["attach", &d, "run.log", "--store", "--write", "--yes"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["guard"][0]["level"], "warn");
}

#[test]
fn clean_and_placeholder_uploads_pass_and_unknown_guards_are_named() {
    let r = common::repo();
    with_store(&r, SECRETS);
    let d = r.new_doc("design", "services/shop", "Runs");
    r.write("run.log", "url postgres://app:${DB_PASSWORD}@db.acme.io/x and mysql://root:hunter22@localhost/x\n");
    let (code, v) = r.json(&["attach", &d, "run.log", "--store", "--write", "--yes"]);
    assert_eq!(code, 0, "{v}");
    let (code, v) = r.json(&["attach", &d, "run.log", "--store", "--allow-hit", "nope"]);
    assert_ne!(code, 0);
    assert!(v.to_string().contains("no guard named 'nope'"), "{v}");
}

fn mcp_show(r: &common::Repo, id: &str) -> serde_json::Value {
    let input = format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{{\"name\":\"spec_show\",\"arguments\":{{\"doc\":\"{id}\",\"body\":true}}}}}}\n"
    );
    let out = r.spec().arg("mcp").write_stdin(input).output().unwrap();
    let line: serde_json::Value =
        serde_json::from_str(String::from_utf8_lossy(&out.stdout).lines().next().unwrap()).unwrap();
    serde_json::from_str(line["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
}

#[test]
fn mcp_redacts_secrets_and_edits_refuse_the_marker() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Database");
    let path = r.json(&["show", &id]).1["path"].as_str().unwrap().to_string();
    r.write(&path, &(r.read(&path) + "\nDSN: " + &leaky_url() + "\n"));
    let v = mcp_show(&r, &id);
    let s = v.to_string();
    assert!(!s.contains("Et-77x"), "{s}");
    assert!(s.contains("[redacted: connection string with a password]"), "{s}");
    assert_eq!(v["redacted"], 1, "{s}");
    let (code, v) = r.json(&["edit", &id, "--section", "Decision", "--text", "DSN: [redacted: JWT]", "--write"]);
    assert_ne!(code, 0, "{v}");
    assert!(v.to_string().contains("spec mcp") && v.to_string().contains("--append"), "{v}");
    assert_eq!(v["next"], "spec help edit", "{v}");
    r.write("spec.toml", &(r.read("spec.toml") + "[mcp]\nredact = false\n"));
    assert!(mcp_show(&r, &id).to_string().contains("Et-77x"), "redaction can be turned off");
}

/// A JWT assembled at run time, in the shape the built-in pattern matches.
fn jwt() -> String {
    let part = |n: usize| "a".repeat(n);
    format!("{}{}.{}{}.{}", "ey", "J".to_string() + &part(20), "ey", "J".to_string() + &part(30), part(43))
}

#[test]
fn a_denied_term_guard_refuses_an_upload_too() {
    let r = common::repo();
    with_store(&r, "[[guards]]\nname = \"terms\"\ndeny_terms = [\"orchid-internal\"]\nrange = \"all\"\n");
    let d = r.new_doc("design", "services/shop", "Runs");
    r.write("run.log", "host Orchid-Internal-7\n");
    let (code, v) = r.json(&["attach", &d, "run.log", "--store", "--write", "--yes"]);
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["guard"][0]["guard"], "terms", "{v}");
    assert_eq!(v["guard"][0]["allowed"], false, "{v}");
    assert!(!v.to_string().contains("Orchid-Internal-7"), "the matched text is never printed: {v}");
    assert_eq!(v["next"], format!("spec attach {d} run.log --store --write --yes --allow-hit terms"));
}

#[test]
fn allowing_one_guard_does_not_let_another_through() {
    let r = common::repo();
    with_store(
        &r,
        &format!("{SECRETS}[[guards]]\nname = \"terms\"\ndeny_terms = [\"orchid-internal\"]\nrange = \"all\"\n"),
    );
    let d = r.new_doc("design", "services/shop", "Runs");
    r.write("run.log", &format!("{}\norchid-internal\n", leaky_url()));
    let (code, v) = r.json(&["attach", &d, "run.log", "--store", "--write", "--yes", "--allow-hit", "secrets"]);
    assert_eq!(code, 1, "{v}");
    let hits = v["guard"].as_array().unwrap();
    assert_eq!(hits.len(), 2, "{v}");
    assert!(hits.iter().any(|h| h["guard"] == "secrets" && h["allowed"] == true), "{v}");
    assert!(hits.iter().any(|h| h["guard"] == "terms" && h["allowed"] == false), "{v}");
    assert!(r.json(&["files", &d]).1.to_string().find("run.log").is_none(), "nothing attached");
}

#[test]
fn an_upload_past_the_scan_limit_says_only_the_start_was_read() {
    let r = common::repo();
    with_store(&r, SECRETS);
    let d = r.new_doc("design", "services/shop", "Runs");
    let big = r.path().join("big.bin");
    let f = std::fs::File::create(&big).unwrap();
    f.set_len(64 * 1024 * 1024 + 1).unwrap();
    let (code, t) = r.text(&["attach", &d, "big.bin", "--store"]);
    assert_eq!(code, 0, "{t}");
    assert!(t.contains("only the first 64 MiB was read by the upload guard"), "{t}");
}

#[test]
fn mcp_counts_each_redaction_and_covers_jwts() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Tokens");
    let path = r.json(&["show", &id]).1["path"].as_str().unwrap().to_string();
    r.write(&path, &format!("{}\nDSN: {}\nBearer {}\n", r.read(&path), leaky_url(), jwt()));
    let v = mcp_show(&r, &id);
    let s = v.to_string();
    assert_eq!(v["redacted"], 2, "{s}");
    assert!(s.contains("[redacted: JWT]") && !s.contains(&jwt()), "{s}");
}

#[test]
fn mcp_leaves_placeholders_and_example_hosts_alone() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Examples");
    let path = r.json(&["show", &id]).1["path"].as_str().unwrap().to_string();
    let text = "DSN: postgres://app:${DB_PASSWORD}@db.acme.io/main and mysql://u:pw-12345@db.example.com/x\n";
    r.write(&path, &(r.read(&path) + text));
    let v = mcp_show(&r, &id);
    assert!(v.get("redacted").is_none(), "no field when nothing was replaced: {v}");
    assert!(v.to_string().contains("pw-12345@db.example.com"), "{v}");
}

#[test]
fn set_refuses_the_marker_and_a_document_that_already_holds_it_can_be_edited() {
    let r = common::repo();
    let id = r.new_doc("adr", "services/shop", "Database");
    let (code, v) = r.json(&["set", &id, "title=[redacted: JWT]"]);
    assert_ne!(code, 0, "{v}");
    assert!(v.to_string().contains("spec mcp"), "{v}");
    let path = r.json(&["show", &id]).1["path"].as_str().unwrap().to_string();
    r.write(&path, &(r.read(&path) + "\nExample output: [redacted: JWT]\n"));
    let (code, v) =
        r.json(&["edit", &id, "--section", "Decision", "--text", "Keep [redacted: JWT] as written.", "--write"]);
    assert_eq!(code, 0, "{v}");
}

#[test]
fn attach_runs_the_built_in_secrets_check_with_no_guard_configured() {
    let r = common::repo();
    with_store(&r, "");
    let d = r.new_doc("design", "services/shop", "Runs");
    r.write("run.log", &format!("connecting to {}\n", leaky_url()));
    let (code, v) = r.json(&["attach", &d, "run.log", "--store", "--write", "--yes"]);
    assert_eq!(code, 1, "a store upload is refused: {v}");
    assert!(!v.to_string().contains("Et-77x"), "{v}");
    assert_eq!(v["next"], format!("spec attach {d} run.log --store --write --yes --allow-hit secrets"));
    let (code, v) = r.json(&["attach", &d, "run.log", "--store", "--write", "--yes", "--allow-hit", "secrets"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["guard"][0]["allowed"], true, "{v}");
    r.write("small.log", &format!("db {}\n", leaky_url()));
    let (code, v) = r.json(&["attach", &d, "small.log", "--repo", "--write"]);
    assert_eq!(code, 0, "a repository file is attached: {v}");
    assert_eq!(v["guard"][0]["level"], "warn", "{v}");
}

#[test]
fn c216_an_unknown_guard_lists_the_built_in_secrets_check() {
    let r = common::repo();
    let (code, v) = r.json(&["guard", "nope"]);
    assert_eq!(code, 2, "{v}");
    assert!(v.to_string().contains("secrets (built-in)"), "{v}");
}
