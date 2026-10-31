//! The README and docs/ checked against the CLI, so they cannot drift from `--help`.

mod common;

use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// The text between `<!-- spec:<name>:begin -->` and `<!-- spec:<name>:end -->`.
fn block(text: &str, name: &str) -> String {
    let (open, close) = (format!("<!-- spec:{name}:begin -->"), format!("<!-- spec:{name}:end -->"));
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
            read(file).contains("<!-- spec:commands:begin -->\n\n|"),
            "{file}: keep a blank line between <!-- spec:commands:begin --> and the table"
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
    // A console block: `$ spec ...` lines, each followed by lines its output must contain
    // (trimmed output is fine; every shown line must appear, with spacing collapsed).
    let mut steps: Vec<(Vec<String>, Vec<String>)> = vec![];
    for l in block(&readme, "quickstart").lines() {
        if let Some(cmd) = l.strip_prefix("$ ") {
            steps.push((words(cmd), vec![]));
        } else if !l.starts_with("```") && !l.trim().is_empty() {
            steps.last_mut().expect("output before the first command").1.push(l.to_string());
        }
    }
    assert!(steps.len() >= 3, "quick start has too few commands");
    let squash = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    for (w, expect) in steps {
        assert_eq!(w[0], "spec");
        let out = r.spec().args(&w[1..]).output().unwrap();
        let code = out.status.code().unwrap_or(-1);
        assert!(code <= 1, "quick start `{}` exited {code}: {}", w.join(" "), String::from_utf8_lossy(&out.stderr));
        let got = squash(&String::from_utf8_lossy(&out.stdout));
        for e in expect {
            assert!(got.contains(&squash(&e)), "quick start `{}` printed no `{e}`:\n{got}", w.join(" "));
        }
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
    let (code, page) = r.text(&["help", "attach", "--markdown"]);
    assert_eq!(code, 0, "{page}");
    assert!(page.contains("```\n[files]\ninline_max_bytes"), "notes keep their config snippet: {page}");
    let (code, text) = r.text(&["help", "attach"]);
    assert_eq!(code, 0, "{text}");
    assert!(text.contains("[files.store]") && text.contains("refuses the upload"), "{text}");
    assert!(!text.contains("Examples:"), "{text}");
    let (_, v) = r.json(&["help", "attach"]);
    assert!(v.to_string().contains("\"notes\""), "{v}");
    let (code, v) = r.json(&["help", "mvv", "--markdown"]);
    assert_ne!(code, 0);
    assert!(v["error"].as_str().unwrap().contains("did you mean 'mv'"), "{v}");
}

#[test]
fn every_argument_has_help() {
    let r = common::repo();
    let (_, v) = r.json(&["help"]);
    for g in v["global"].as_array().unwrap() {
        assert!(!g["help"].as_str().unwrap_or("").trim().is_empty(), "global {} has no help", g["name"]);
    }
    for c in v["items"].as_array().unwrap() {
        assert!(!c["about"].as_str().unwrap().trim().is_empty(), "{} has no summary", c["name"]);
        for a in c["args"].as_array().unwrap() {
            assert!(
                !a["help"].as_str().unwrap_or("").trim().is_empty(),
                "spec {} {} has no help",
                c["name"],
                a["name"]
            );
        }
    }
}

#[test]
fn llms_txt_matches_help_llms() {
    let r = common::repo();
    let (code, text) = r.text(&["help", "--llms"]);
    assert_eq!(code, 0);
    assert_eq!(read("llms.txt"), text, "llms.txt is stale: spec help --llms > llms.txt");
    let cargo = read("crates/specfold-cli/Cargo.toml");
    let desc = cargo.lines().find_map(|l| l.strip_prefix("description = ")).unwrap().trim_matches('"');
    assert!(text.contains(&format!("> {desc}")), "{text}");
}

#[test]
fn readme_stays_short() {
    let n = read("README.md").lines().count();
    assert!(n <= 150, "README.md has {n} lines; move detail to docs/");
}

#[test]
fn find_aliases_are_listed_in_help() {
    let r = common::repo();
    let (_, table) = r.text(&["help", "--markdown"]);
    assert!(
        table.contains(
            "| `spec find` | Find documents by words, type, owner, status or tag (also `spec list`, `spec ls`) |"
        ),
        "{table}"
    );
    let (_, v) = r.json(&["help"]);
    let find = v["items"].as_array().unwrap().iter().find(|c| c["name"] == "find").unwrap().clone();
    assert_eq!(find["aliases"], serde_json::json!(["list", "ls"]));
}

#[test]
fn every_rule_is_in_the_rules_reference() {
    let r = common::repo();
    let (_, v) = r.json(&["explain"]);
    let page = read("docs/reference/rules.md");
    for item in v["items"].as_array().unwrap() {
        let id = item["id"].as_str().unwrap();
        assert!(page.contains(&format!("| **{id}** |")), "docs/reference/rules.md has no row for {id}");
    }
}

/// Bare `--flag` words outside code in markdown prose: Jekyll typography turns `--` into an en dash.
fn bare_flags(text: &str) -> Vec<String> {
    let (mut out, mut fence, mut code) = (vec![], false, false);
    for (n, line) in text.lines().enumerate() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            fence = !fence;
            continue;
        }
        if fence || line.starts_with("    ") || line.trim().starts_with("<!--") {
            continue;
        }
        if line.trim().is_empty() {
            code = false;
        }
        let chars: Vec<char> = line.chars().collect();
        for i in 0..chars.len() {
            if chars[i] == '`' {
                code = !code;
            } else if !code
                && chars[i] == '-'
                && chars.get(i + 1) == Some(&'-')
                && chars.get(i + 2).is_some_and(char::is_ascii_lowercase)
                && (i == 0 || !(chars[i - 1].is_alphanumeric() || chars[i - 1] == '-'))
            {
                out.push(format!("line {}: {line}", n + 1));
            }
        }
    }
    out
}

#[test]
fn c218_flags_in_prose_are_code_spans() {
    let r = common::repo();
    for args in [
        &["help", "--markdown"][..],
        &["help", "--llms"],
        &["help", "edit", "--markdown"],
        &["help", "sync", "--markdown"],
    ] {
        let (code, text) = r.text(args);
        assert_eq!(code, 0, "{text}");
        assert!(bare_flags(&text).is_empty(), "spec {args:?}: {:?}", bare_flags(&text));
    }
    let mut files = vec![root().join("README.md")];
    let mut dirs = vec![root().join("docs")];
    while let Some(d) = dirs.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                dirs.push(p);
            } else if p.extension().is_some_and(|x| x == "md") {
                files.push(p);
            }
        }
    }
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        assert!(bare_flags(&text).is_empty(), "{}: {:?}", f.display(), bare_flags(&text));
    }
}
