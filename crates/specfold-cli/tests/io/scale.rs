//! Large-repository regression: a synthetic corpus must stay fast enough for CI and hooks.
//! Bounds leave room for debug builds on shared runners yet catch a regression; the ignored test prints timings at 10k.
#[path = "../common/mod.rs"]
mod common;

use std::time::{Duration, Instant};

/// `owners` services with `per` linked documents each; every document links to the previous
/// one in its owner and to the same position in the previous owner.
fn corpus(owners: usize, per: usize) -> common::Repo {
    let r = common::repo();
    for o in 0..owners {
        r.write(&format!("services/s{o}/src/lib.rs"), "fn f() {}\n");
        for j in 0..per {
            let mut body = format!("\n# Doc {o} {j}\n\n## Rule\n\nKeep it.\n\n## Why\n\n");
            if j > 0 {
                body += &format!("See [[sot:s{o}/d{}]].\n", j - 1);
            }
            if o > 0 {
                body += &format!("Also [[sot:s{}/d{j}]].\n", o - 1);
            }
            body += "\n## Verification\n\nTests.\n";
            r.write(
                &format!("services/s{o}/.spec/sot/d{j}.sot.md"),
                &format!(
                    "---\nid: sot:s{o}/d{j}\ntitle: Doc {o} {j}\nstatus: draft\ncreated: 2026-01-01T00:00:00Z\nupdated: 2026-01-01T00:00:00Z\nrel:\n  governs: [services/s{o}]\n---\n{body}"
                ),
            );
        }
    }
    r.git(&["add", "-A"]);
    r
}

fn timed(r: &common::Repo, args: &[&str], bound: Duration) -> (i32, serde_json::Value, Duration) {
    let t = Instant::now();
    let (code, v) = r.json(args);
    let took = t.elapsed();
    assert!(took < bound, "spec {} took {took:?}, bound {bound:?}", args.join(" "));
    (code, v, took)
}

const COMMANDS: &[&[&str]] = &[
    &["check"],
    &["census"],
    &["graph"],
    &["graph", "impact", "sot:s0/d0"],
    &["audit"],
    &["find", "doc"],
    &["doctor"],
];

#[test]
fn a_thousand_documents_stay_within_bounds() {
    let r = corpus(25, 40);
    let bound = Duration::from_secs(10);
    let (code, v, _) = timed(&r, &["check"], bound);
    assert_eq!(code, 0, "the synthetic corpus is clean: {v}");
    let (_, v, _) = timed(&r, &["census"], bound);
    assert!(v.to_string().contains("1000"), "census sees every document: {v}");
    let (_, v, _) = timed(&r, &["graph", "impact", "sot:s0/d0"], bound);
    assert!(v["total"].as_u64().unwrap() > 100, "impact walks the deep chain: {v}");
    for args in COMMANDS {
        let (code, v, _) = timed(&r, args, bound);
        assert!(code <= 1, "spec {} failed: {v}", args.join(" "));
    }
}

/// A folder of 3,000 governed files turned into a submodule holding the same files, staged.
fn gitlink_conversion() -> (common::Repo, common::Repo) {
    let source = common::Repo::new(tempfile::tempdir().unwrap());
    source.git(&["init", "-q", "-b", "main"]);
    source.git(&["config", "user.email", "test@example.com"]);
    source.git(&["config", "user.name", "tester"]);
    for i in 0..3000 {
        source.write(&format!("p{}/f{i}.rs", i / 100), "fn g() {}\n");
    }
    source.commit("gen");
    let r = common::repo();
    let globs = ["services/s{o}", "services/s{o}/src/**/*.rs", "services/s{o}/gen/**", "*.toml", "services/*/gen/**"];
    for o in 0..20 {
        for j in 0..50 {
            let g = globs[j % globs.len()].replace("{o}", &o.to_string());
            r.write(
                &format!("services/s{o}/.spec/sot/d{j}.sot.md"),
                &format!(
                    "---\nid: sot:s{o}/d{j}\ntitle: Doc {o} {j}\nstatus: active\ncreated: 2026-01-01T00:00:00Z\nupdated: 2026-01-01T00:00:00Z\nrel:\n  governs: ['{g}']\n---\n\n# Doc {o} {j}\n"
                ),
            );
        }
    }
    for i in 0..3000 {
        r.write(&format!("services/s0/gen/p{}/f{i}.rs", i / 100), "fn g() {}\n");
    }
    r.commit("corpus");
    r.git(&["rm", "-r", "-q", "services/s0/gen"]);
    let url = source.path().to_str().unwrap();
    r.git(&["-c", "protocol.file.allow=always", "submodule", "add", "-q", url, "services/s0/gen"]);
    (r, source)
}

#[test]
fn d1_and_verify_on_a_three_thousand_file_gitlink_conversion_stay_within_bounds() {
    let (r, _source) = gitlink_conversion();
    // Before the governs index this took minutes; the bound leaves room for debug builds.
    let bound = Duration::from_secs(60);
    let (code, v, _) = timed(&r, &["check", "--staged", "--rule", "D1"], bound);
    assert_eq!(code, 0, "D1 is a warning: {v}");
    // The files moved into the submodule unchanged: one split, nothing removed or changed.
    assert!(v["by_rule"].get("D1").is_none(), "{v}");
    let split = &v["gitlinks"][0];
    assert_eq!((split["path"].as_str(), split["kind"].as_str()), (Some("services/s0/gen"), Some("split")), "{v}");
    assert_eq!((split["moved"].as_u64(), split["changed"].as_u64()), (Some(3000), Some(0)), "{v}");
    r.git(&["commit", "-qm", "gitlink"]);
    let (code, v, _) = timed(&r, &["verify", "--changed", "HEAD~1"], bound);
    assert_eq!(code, 0, "{v}");
}

#[test]
#[ignore = "benchmark: cargo test --release --test scale -- --ignored --nocapture"]
fn ten_thousand_documents_timings() {
    let r = corpus(100, 100);
    for args in COMMANDS {
        let (code, _, took) = timed(&r, args, Duration::from_secs(300));
        println!("spec {:<28} exit {code}  {took:?}", args.join(" "));
    }
}
