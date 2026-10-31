//! Repository conventions (AGENTS.md CLI-18, CLI-21) checked over the source tree.

use std::path::{Path, PathBuf};

/// Every `.rs` file under `dir`.
fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    // A crate without a `tests/` directory has nothing to walk.
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries {
        let p = e.expect("dir entry").path();
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// The `.rs` files under `<crate>/<sub>` for both crates.
fn files(sub: &str) -> Vec<PathBuf> {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut out = vec![];
    walk(&crates.join("specfold-cli").join(sub), &mut out);
    walk(&crates.join("specfold-core").join(sub), &mut out);
    out
}

/// CLI-21: every source file in both crates opens with a `//!` module doc.
#[test]
fn every_source_module_has_a_module_doc() {
    let missing: Vec<_> = files("src")
        .into_iter()
        .filter(|p| !std::fs::read_to_string(p).expect("source reads").trim_start().starts_with("//!"))
        .collect();
    assert!(missing.is_empty(), "modules without //!: {missing:?}");
}

/// CLI-18: source files stay within 500 lines and test files within 1600.
#[test]
fn files_stay_within_the_length_limits() {
    let over = |sub: &str, max: usize| -> Vec<(PathBuf, usize)> {
        files(sub)
            .into_iter()
            .map(|p| {
                let n = std::fs::read_to_string(&p).expect("file reads").lines().count();
                (p, n)
            })
            .filter(|(_, n)| *n > max)
            .collect()
    };
    assert_eq!(over("src", 500), vec![], "src files over 500 lines");
    assert_eq!(over("tests", 1600), vec![], "test files over 1600 lines");
}

/// C227: every `spec set` a message, fix, explain or help names carries `--write`, so it writes
/// once set previews by default.
#[test]
fn every_suggested_set_writes() {
    let ends = ["`", " #", " && ", "; ", ", then", ")\"", ") ", "\\n", "\"", "\n", ", or", ", which"];
    // A quoted argument (`'title=<text>'`, `\"…\"`) is part of the command, not its end.
    let unquote = |t: &str| t.replace("\\\"", "").replace('\'', "");
    let mut bad = vec![];
    for f in files("src") {
        let s = std::fs::read_to_string(&f).expect("read source");
        for (j, _) in s.match_indices("spec set ") {
            let rest = unquote(&s[j + 9..]);
            let stop = ends.iter().filter_map(|e| rest.find(e)).min().unwrap_or(rest.len());
            if !rest[..stop].contains("--write") {
                bad.push(format!("{}: spec set {}", f.display(), &rest[..stop]));
            }
        }
    }
    assert!(bad.is_empty(), "spec set without --write: {bad:#?}");
}

/// C229: an option value quoted into a suggested command goes through `shell::opt`, so a value
/// starting with `-` is written `--opt=value` and not read as another option.
#[test]
fn quoted_option_values_go_through_opt() {
    let mut bad = vec![];
    for f in files("src") {
        let s = std::fs::read_to_string(&f).expect("read source");
        for (n, l) in s.lines().enumerate() {
            let t = l.replace("specfold_core::shell::", "").replace("crate::shell::", "");
            let Some(at) = t.find(" {}\", quote(") else { continue };
            let flag = t[..at].rsplit([' ', '"']).next().unwrap_or("");
            if flag.starts_with("--") && !["--write", "--yes", "--force", "--all"].contains(&flag) {
                bad.push(format!("{}:{}: {}", f.display(), n + 1, l.trim()));
            }
        }
    }
    assert!(bad.is_empty(), "use shell::opt for these: {bad:#?}");
}
