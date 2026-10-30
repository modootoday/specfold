//! Choosing an imported document's owner: from what it says it governs, else from the code paths
//! its text mentions most.

use crate::placement;
use crate::workspace::{Owner, Owners};
use regex::Regex;
use std::collections::BTreeMap;
use std::sync::OnceLock;

fn path_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"(?:^|[\s`(\[])((?:[A-Za-z0-9_.@-]+/){1,6}[A-Za-z0-9_.@-]+)").expect("static regex is valid")
    })
}

/// Returns the owner and a reason. A single owner must hold at least 60% of the mentions;
/// without that evidence the document stays with `home`, the owner it already lives in.
pub fn choose<'a>(owners: &'a Owners, governs: &[String], text: &str, home: &'a Owner) -> (&'a Owner, String) {
    if !governs.is_empty() {
        let d = placement::decide(owners, governs, owners.root());
        return (d.owner, d.reason);
    }
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut total = 0;
    for c in path_re().captures_iter(text) {
        let p = c[1].trim_start_matches("./");
        let o = owners.containing(p);
        if o.path.is_empty() {
            continue;
        }
        *counts.entry(o.id.clone()).or_default() += 1;
        total += 1;
    }
    let best = counts.iter().max_by_key(|(_, n)| **n);
    match best {
        Some((id, n)) if *n * 10 >= total * 6 => {
            let o = owners.by_id(id).unwrap_or(owners.root());
            let stored = owners.storage_owner(o);
            (stored, format!("{n} of {total} code paths in the text are under {}", o.path))
        }
        _ => {
            let why = if total == 0 {
                "mentions no owned code".to_string()
            } else {
                format!("{total} code paths spread over {} owners", counts.len())
            };
            (home, format!("{why}; stays with {}", home.id))
        }
    }
}
