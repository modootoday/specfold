//! Large-repository regression: a synthetic corpus must stay fast enough for CI and hooks.
//! Bounds are generous (debug build, shared runners); the ignored test prints timings at 10k.
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
    let bound = Duration::from_secs(30);
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

#[test]
#[ignore = "benchmark: cargo test --release --test scale -- --ignored --nocapture"]
fn ten_thousand_documents_timings() {
    let r = corpus(100, 100);
    for args in COMMANDS {
        let (code, _, took) = timed(&r, args, Duration::from_secs(300));
        println!("spec {:<28} exit {code}  {took:?}", args.join(" "));
    }
}
