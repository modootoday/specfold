//! The README and docs/ checked against the CLI, so they cannot drift from `--help`.

mod common;

use assert_cmd::Command;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// The text between `<!-- spec:<name> -->` and `<!-- /spec:<name> -->`.
fn block(text: &str, name: &str) -> String {
    let (open, close) = (format!("<!-- spec:{name} -->"), format!("<!-- /spec:{name} -->"));
    let start = text.find(&open).unwrap_or_else(|| panic!("no {open}")) + open.len();
    let end = text[start..].find(&close).unwrap_or_else(|| panic!("no {close}")) + start;
    text[start..end].trim().to_string()
}

/// Splits a shell line into words, honouring single and double quotes.
fn words(line: &str) -> Vec<String> {
    let (mut out, mut cur, mut quote, mut any) = (vec![], String::new(), None, false);
    for ch in line.chars() {
        match (quote, ch) {
            (None, '\'' | '"') => (quote, any) = (Some(ch), true),
            (Some(q), c) if c == q => quote = None,
            (None, c) if c.is_whitespace() => {
                if any {
                    out.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            (_, c) => (cur.push(c), any = true).1,
        }
    }
    if any {
        out.push(cur);
    }
    out
}

#[test]
fn command_tables_match_help_markdown() {
    let r = common::repo();
    let (code, table) = r.text(&["help", "--markdown"]);
    assert_eq!(code, 0);
    for file in ["README.md", "docs/reference/commands.md"] {
        assert_eq!(
            block(&read(file), "commands"),
            table.trim(),
            "{file} is stale: replace its spec:commands block with `spec help --markdown`"
        );
        assert!(
            read(file).contains("<!-- spec:commands -->\n\n|"),
            "{file}: keep a blank line between <!-- spec:commands --> and the table"
        );
    }
    assert!(table.starts_with("\n|"), "help --markdown must open with a blank line: {table:?}");
}

#[test]
fn quick_start_runs() {
    let dir = tempfile::tempdir().unwrap();
    let r = common::Repo { dir };
    r.git(&["init", "-q", "-b", "main"]);
    r.write("services/shop/src/cart/total.rs", "fn total() {}\n");
    let readme = read("README.md");
    let lines: Vec<_> = block(&readme, "quickstart").lines().filter_map(|l| l.strip_prefix("$ ")).map(words).collect();
    assert!(lines.len() >= 3, "quick start has too few commands");
    for w in lines {
        assert_eq!(w[0], "spec");
        let out = r.spec().args(&w[1..]).output().unwrap();
        let code = out.status.code().unwrap_or(-1);
        assert!(code <= 1, "quick start `{}` exited {code}: {}", w.join(" "), String::from_utf8_lossy(&out.stderr));
    }
}

#[test]
fn every_command_has_an_example() {
    let r = common::repo();
    let (_, v) = r.json(&["help"]);
    for c in v["items"].as_array().unwrap() {
        assert!(!c["examples"].as_array().unwrap().is_empty(), "{} has no example in its --help", c["name"]);
    }
}

#[test]
fn descriptions_agree() {
    let readme = block(&read("README.md"), "description");
    let cargo = read("crates/specfold-cli/Cargo.toml");
    let cargo = cargo.lines().find_map(|l| l.strip_prefix("description = ")).unwrap().trim_matches('"');
    let npm: serde_json::Value = serde_json::from_str(&read("npm/package.json")).unwrap();
    assert_eq!(readme, cargo, "README and Cargo.toml descriptions differ");
    assert_eq!(npm["description"], cargo, "npm and Cargo.toml descriptions differ");
}

#[test]
fn readme_links_resolve() {
    let readme = read("README.md");
    for part in readme.split("](").skip(1) {
        let target = &part[..part.find(')').unwrap()];
        if target.starts_with("http") || target.starts_with('#') {
            continue;
        }
        assert!(root().join(target).exists(), "README links to missing {target}");
    }
}

#[test]
fn help_markdown_for_one_command_is_a_reference_page() {
    let r = common::repo();
    let (code, page) = r.text(&["help", "mv", "--markdown"]);
    assert_eq!(code, 0, "{page}");
    assert!(page.starts_with("\n## `spec mv`\n"), "{page}");
    assert!(page.contains("| `--slug` |") && page.contains("```sh"), "{page}");
    let (code, v) = r.json(&["help", "mvv", "--markdown"]);
    assert_ne!(code, 0);
    assert!(v["error"].as_str().unwrap().contains("did you mean 'mv'"), "{v}");
}

/// spec.toml lists README.md and docs/ in [check] count_paths, so a stale count marker in the
/// published pages fails here before it ships.
#[test]
fn published_counts_are_current() {
    let cache = std::env::temp_dir().join(format!("specfold-docs-cache-{}", std::process::id()));
    let out = Command::cargo_bin("spec")
        .unwrap()
        .current_dir(root())
        .env("SPECFOLD_CACHE", &cache)
        .args(["check", "--rule", "N1", "--no-local"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(text.contains("0 error(s), 0 warning(s)"), "{text}");
    let _ = std::fs::remove_dir_all(cache);
}
