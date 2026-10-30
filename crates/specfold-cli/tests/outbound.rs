//! Content leaving the repository: the guard before `spec attach` uploads to the store, and
//! redaction of secrets in what `spec mcp` hands an agent (S2, S4).

mod common;

fn leaky_url() -> String {
    format!("postgres://app:{}{}@db.acme.io/main", "s3cr", "Et-77x")
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
    let (code, v) = r.json(&["attach", &d, "run.log", "--store", "--write"]);
    assert_eq!(code, 1, "{v}");
    let s = v.to_string();
    assert!(s.contains("connection string") && !s.contains("Et-77x"), "{s}");
    assert_eq!(v["next"], format!("spec attach {d} run.log --store --write --allow-hit secrets"));
    assert!(r.json(&["files", &d]).1.to_string().find("run.log").is_none(), "nothing attached");
    let (code, v) = r.json(&["attach", &d, "run.log", "--store", "--write", "--allow-hit", "secrets"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["guard"][0]["allowed"], true);
}

#[test]
fn a_warn_guard_shows_its_hit_and_uploads() {
    let r = common::repo();
    with_store(&r, &format!("{SECRETS}level = \"warn\"\n"));
    let d = r.new_doc("design", "services/shop", "Runs");
    r.write("run.log", &format!("connecting to {}\n", leaky_url()));
    let (code, v) = r.json(&["attach", &d, "run.log", "--store", "--write"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["guard"][0]["level"], "warn");
}

#[test]
fn clean_and_placeholder_uploads_pass_and_unknown_guards_are_named() {
    let r = common::repo();
    with_store(&r, SECRETS);
    let d = r.new_doc("design", "services/shop", "Runs");
    r.write("run.log", "url postgres://app:${DB_PASSWORD}@db.acme.io/x and mysql://root:hunter22@localhost/x\n");
    let (code, v) = r.json(&["attach", &d, "run.log", "--store", "--write"]);
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
    assert!(v["redacted"].as_u64().unwrap() >= 1, "{s}");
    let (code, v) = r.json(&["edit", &id, "--section", "Decision", "--text", "DSN: [redacted: JWT]", "--write"]);
    assert_ne!(code, 0, "{v}");
    assert!(v.to_string().contains("spec mcp"), "{v}");
    r.write("spec.toml", &(r.read("spec.toml") + "[mcp]\nredact = false\n"));
    assert!(mcp_show(&r, &id).to_string().contains("Et-77x"), "redaction can be turned off");
}
