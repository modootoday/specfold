//! The alias table an import writes: old names to the ids they now resolve to.

use std::collections::BTreeSet;

/// Appends the alias rows to the table file (creating it), keeping one row per old name.
pub fn alias_table(existing: &str, rows: &[(String, String)]) -> String {
    let mut seen: BTreeSet<String> = existing.lines().map(str::to_string).collect();
    let mut out = if existing.is_empty() {
        "# old\tnew-id  (written by spec import; frozen afterwards)\n".to_string()
    } else {
        existing.to_string()
    };
    for (old, new) in rows {
        let line = format!("{old}\t{new}");
        if seen.insert(line.clone()) {
            out.push_str(&line);
            out.push('\n');
        }
    }
    out
}
