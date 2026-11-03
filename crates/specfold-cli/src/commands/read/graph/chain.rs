//! `spec graph chain`: the supersede chain through a document.

use crate::output::Output;
use crate::session::{self, Session};
use anyhow::Result;
use serde_json::{Value, json};
use specfold_core::graph::query;
use std::collections::BTreeSet;

/// `chain`: the supersede chain through the target.
pub fn chain(s: &Session, t: &str, mut o: Output) -> Result<Output> {
    let c = &s.corpus;
    let id = |i: usize| c.docs[i].id.to_string();
    let idx = session::resolve(s, t)?;
    let mut chain = older(s, idx);
    chain.reverse();
    chain.extend(query::chain(&s.graph, idx));
    // C213: a supersede cycle repeats ids; walk it once from the target and name no current.
    let mut seen = BTreeSet::new();
    let cycle = !chain.iter().all(|i| seen.insert(*i));
    if cycle {
        let mut seen = BTreeSet::new();
        chain = query::chain(&s.graph, idx).into_iter().take_while(|i| seen.insert(*i)).collect();
    }
    for (n, i) in chain.iter().enumerate() {
        let this = if *i == idx { "  (this)" } else { "" };
        o = o.item(
            json!({"id": id(*i), "status": c.docs[*i].status(), "this": *i == idx}),
            format!("{}{}{this}", if n == 0 { "" } else { "superseded by " }, id(*i)),
        );
    }
    if cycle {
        return Ok(o
            .field("current", Value::Null)
            .foot("a supersede cycle, nothing is current")
            .next("spec check --rule T1"));
    }
    let last = id(*chain.last().unwrap_or(&idx));
    Ok(o.field("current", last.clone()).next(format!("spec show {}", specfold_core::shell::word(&last))))
}

/// The documents `idx` supersedes, nearest first.
fn older(s: &Session, idx: usize) -> Vec<usize> {
    let (mut out, mut cur) = (vec![], idx);
    while let Some(n) = s.graph.edges_from(cur).find(|e| e.kind == "supersedes").map(|e| e.to) {
        if n == idx || out.contains(&n) {
            break;
        }
        out.push(n);
        cur = n;
    }
    out
}
