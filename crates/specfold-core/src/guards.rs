//! Guards: named checks over any file in the repository and over commit messages, such as the
//! gate that keeps internal names out of a public package.

mod code_spans;
mod compile;
mod jslex;
mod pattern;
mod prove;
pub mod range;
mod scan;
pub mod secrets;
pub mod terms;

use crate::check::baseline;
use crate::check::finding::Finding;
use crate::config::{Config, GuardDef};
use anyhow::{Result, anyhow, bail};
pub use range::GuardRange;

fn select<'a>(cfg: &'a Config, names: &[String]) -> Result<Vec<&'a GuardDef>> {
    if names.is_empty() {
        return Ok(cfg.raw.guards.iter().collect());
    }
    names
        .iter()
        .map(|n| {
            cfg.raw.guards.iter().find(|g| &g.name == n).ok_or_else(|| {
                let have: Vec<&str> = cfg.raw.guards.iter().map(|g| g.name.as_str()).collect();
                anyhow!("no guard named '{n}' (have: {})", have.join(", "))
            })
        })
        .collect()
}

fn scan_one(cfg: &Config, g: &GuardDef, range: Option<&GuardRange>) -> Result<scan::Scanned> {
    let c = compile::compile(cfg, g)?;
    prove::prove(g, &c)?;
    let r = range.cloned().unwrap_or_else(|| GuardRange::parse(&g.range));
    scan::run(cfg, g, &c, &range::resolve(&cfg.root, &r)?)
}

/// Runs the named guards (all of them when `names` is empty). A guard without an explicit range
/// uses its own default; a guard with a baseline reports only hits the baseline does not hold.
pub fn run(cfg: &Config, names: &[String], range: Option<&GuardRange>) -> Result<Vec<Finding>> {
    let mut out = vec![];
    for g in select(cfg, names)? {
        let found = scan_one(cfg, g, range)?.findings;
        match &g.baseline {
            Some(rel) => match baseline::load_file(&cfg.root, rel)? {
                Some(b) => out.extend(baseline::hold_with(&b, found, &[]).findings),
                None => out.extend(found),
            },
            None => out.extend(found),
        }
    }
    Ok(out)
}

/// Measures each named guard over the whole tree and writes its baseline file.
pub fn write_baselines(cfg: &Config, names: &[String]) -> Result<Vec<(String, String, usize)>> {
    let mut written = vec![];
    for g in select(cfg, names)? {
        let Some(rel) = &g.baseline else {
            if !names.is_empty() {
                bail!("guard {} has no baseline file; set baseline = \"…\" on it", g.name);
            }
            continue;
        };
        let scanned = scan_one(cfg, g, Some(&GuardRange::All))?;
        let b = baseline::measure_with(&cfg.root, &scanned.findings, &[], |p| scanned.files.iter().any(|f| f == p))?;
        baseline::write_file(&cfg.root, rel, &b)?;
        written.push((g.name.clone(), rel.clone(), b.counts.values().sum()));
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RawConfig;

    fn config(root: &std::path::Path, guard: &str) -> Config {
        let raw: RawConfig = toml::from_str(&format!("[[guards]]\n{guard}")).unwrap();
        Config::from_raw(root.to_path_buf(), raw, false)
    }

    #[test]
    fn scans_tree() {
        let tmp = tempfile::tempdir().unwrap();
        let r = tmp.path();
        std::fs::create_dir_all(r.join("libs/open/src")).unwrap();
        std::fs::create_dir_all(r.join("services/shop")).unwrap();
        let fake_key = format!("{}{}", "AK", "IA".to_string() + &"X".repeat(16));
        std::fs::write(r.join("libs/open/src/a.rs"), format!("// built for Acme-Internal\nlet k = \"{fake_key}\";\n"))
            .unwrap();
        std::fs::write(r.join("libs/open/.env"), "X=1\n").unwrap();
        std::fs::write(r.join("services/shop/b.rs"), "acme-internal is fine here\n").unwrap();
        std::fs::write(r.join("terms.txt"), "# private\nacme-internal\n").unwrap();
        let cfg = config(
            r,
            "name = \"public\"\npaths = [\"libs/open/**\"]\nrange = \"all\"\ndeny_terms_file = \"terms.txt\"\ndeny_files = [\"**/.env*\"]\nsecrets = true\n",
        );
        let mut f = run(&cfg, &[], None).unwrap();
        f.sort_by_key(|a| (a.path.clone(), a.line));
        let got: Vec<(String, usize)> = f.iter().map(|x| (x.path.clone(), x.line)).collect();
        assert_eq!(
            got,
            [
                ("libs/open/.env".to_string(), 1),
                ("libs/open/src/a.rs".to_string(), 1),
                ("libs/open/src/a.rs".to_string(), 2)
            ]
        );
        assert!(!f[1].message.contains("acme-internal"));
    }

    #[test]
    fn patterns_skip_urls_exemptions_and_allowed_text() {
        let tmp = tempfile::tempdir().unwrap();
        let r = tmp.path();
        std::fs::create_dir_all(r.join("pub/docs")).unwrap();
        std::fs::write(
            r.join("pub/a.md"),
            "see https://example.com/internal/x\ninternal/build.sh\nthe internal/README example\n",
        )
        .unwrap();
        std::fs::write(r.join("pub/docs/b.md"), "internal/build.sh\n").unwrap();
        let cfg = config(
            r,
            "name = \"g\"\npaths = [\"pub/**\"]\nrange = \"all\"\nallow = [\"internal/readme\"]\n[[guards.deny_regex]]\nre = 'internal/[a-z.]+'\nlabel = \"internal path\"\nnot_in_url = true\nexempt = [\"pub/docs/**\"]\nmust_hit = [\"run internal/build.sh\"]\nmust_pass = [\"https://example.com/internal/x\"]\n",
        );
        let f = run(&cfg, &[], None).unwrap();
        let got: Vec<(String, usize)> = f.iter().map(|x| (x.path.clone(), x.line)).collect();
        assert_eq!(got, [("pub/a.md".to_string(), 2)], "{f:?}");
        assert!(f[0].message.contains("internal path"));
    }

    #[test]
    fn a_fixture_that_no_longer_holds_stops_the_run() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = config(
            tmp.path(),
            "name = \"g\"\nrange = \"all\"\n[[guards.deny_regex]]\nre = 'secret-[0-9]+'\nmust_hit = [\"secret-abc\"]\n",
        );
        let e = run(&cfg, &[], None).unwrap_err().to_string();
        assert!(e.contains("does not match its must_hit fixture"), "{e}");
    }

    #[test]
    fn a_whole_tree_run_over_nothing_fails_and_gzip_is_read() {
        use std::io::Write;
        let tmp = tempfile::tempdir().unwrap();
        let r = tmp.path();
        let none = config(r, "name = \"g\"\npaths = [\"missing/**\"]\nrange = \"all\"\ndeny_terms = [\"x\"]\n");
        assert!(run(&none, &[], None).unwrap()[0].message.contains("scanned no file"));
        std::fs::create_dir_all(r.join("pkg")).unwrap();
        let mut gz = flate2::write::GzEncoder::new(vec![], flate2::Compression::default());
        gz.write_all(b"hello acme-internal\n").unwrap();
        std::fs::write(r.join("pkg/a.txt.gz"), gz.finish().unwrap()).unwrap();
        let cfg = config(
            r,
            "name = \"g\"\npaths = [\"pkg/**\"]\nrange = \"all\"\ndeny_terms = [\"acme-internal\"]\ndecompress = true\n",
        );
        assert_eq!(run(&cfg, &[], None).unwrap().len(), 1);
    }

    #[test]
    fn an_empty_or_missing_published_path_is_a_hit() {
        let tmp = tempfile::tempdir().unwrap();
        let r = tmp.path();
        std::fs::create_dir_all(r.join("pub/full")).unwrap();
        std::fs::create_dir_all(r.join("pub/empty")).unwrap();
        std::fs::write(r.join("pub/full/a.md"), "fine\n").unwrap();
        let raw: RawConfig = toml::from_str(
            "[public]\npaths = [\"pub/full\", \"pub/empty\", \"pub/gone\"]\n[[guards]]\nname = \"g\"\npublic = true\nrange = \"all\"\ndeny_terms = [\"x-internal\"]\n",
        )
        .unwrap();
        let cfg = Config::from_raw(r.to_path_buf(), raw, false);
        let mut got: Vec<(String, bool)> =
            run(&cfg, &[], None).unwrap().into_iter().map(|f| (f.path, f.message.contains("not present"))).collect();
        got.sort();
        assert_eq!(got, [("pub/empty".to_string(), false), ("pub/gone".to_string(), true)]);
    }
}
