//! C246: a document is superseded once. Writes that would give it a second successor are refused
//! and point at the current one instead.

use crate::corpus::Corpus;
use anyhow::{Result, bail};

/// The same-type documents whose `rel.supersedes` names `idx` (C204: other types are L3 errors).
pub fn successors(c: &Corpus, idx: usize) -> Vec<usize> {
    let d = &c.docs[idx];
    let me = d.id.to_string();
    (0..c.docs.len())
        .filter(|&i| i != idx && c.docs[i].id.ty == d.id.ty)
        .filter(|&i| {
            c.docs[i].rel().get("supersedes").is_some_and(|l| l.iter().any(|x| x.trim_matches(['[', ']']) == me))
        })
        .collect()
}

/// The end of the supersede chain from `idx`: the document now in force (stops on a fork or cycle).
pub fn current(c: &Corpus, idx: usize) -> usize {
    let mut at = idx;
    let mut seen = vec![idx];
    while let [only] = successors(c, at)[..] {
        if seen.contains(&only) {
            break;
        }
        seen.push(only);
        at = only;
    }
    at
}

/// The way on from a superseded document: change or supersede the one now in force.
pub fn way_on(c: &Corpus, idx: usize) -> Option<String> {
    successors(c, idx).first()?;
    let cur = &c.docs[current(c, idx)];
    let id = crate::suggest::shell::word(cur.id.to_string());
    let title = cur.title.clone().unwrap_or_else(|| cur.slug.replace('-', " ")).replace('\'', "");
    let edit = format!("spec edit {id} --text '…' --write");
    // Only an active document can be superseded (a draft is changed in place).
    if cur.status().as_deref() != Some("active") {
        return Some(format!("the current document is {id}, still {}; next: {edit}", cur.status().unwrap_or_default()));
    }
    Some(format!(
        "the document in force is {id}: change it with `{edit}`, or supersede it; next: spec new {} --from {id} --title '{title}' --write",
        cur.id.ty
    ))
}

/// Refuses making `by` (or a new document when None) a successor of `idx` when another
/// document already supersedes it.
pub fn refuse_second(c: &Corpus, idx: usize, by: Option<usize>) -> Result<()> {
    if successors(c, idx).iter().all(|&s| Some(s) == by) {
        return Ok(());
    }
    let taken: Vec<String> =
        successors(c, idx).iter().filter(|&&s| Some(s) != by).map(|&s| c.docs[s].id.to_string()).collect();
    let way = way_on(c, idx).unwrap_or_default();
    bail!("{} is already superseded by {}; a second successor would fail T1; {way}", c.docs[idx].id, taken.join(", "))
}
