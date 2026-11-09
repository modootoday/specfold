//! The gitlinks a staged or ranged `spec check` read as one event each: a split or a pin bump.

use specfold_core::workspace::vcs::GitlinkMove;

fn kind(m: &GitlinkMove) -> &'static str {
    match (&m.old, m.is_split()) {
        (Some(_), _) => "bump",
        (None, true) => "split",
        (None, false) => "added",
    }
}

/// `gitlinks` in the JSON summary: path, kind (split, bump, added), the two commits and counts.
pub(super) fn json(moves: &[GitlinkMove]) -> serde_json::Value {
    moves
        .iter()
        .map(|m| {
            serde_json::json!({
                "path": m.path, "kind": kind(m), "old": m.old, "new": m.new,
                "moved": m.moved.len(), "changed": m.changed.len(),
            })
        })
        .collect()
}

/// One foot line per gitlink.
pub(super) fn lines(moves: &[GitlinkMove]) -> Vec<String> {
    moves
        .iter()
        .map(|m| match kind(m) {
            "split" => format!(
                "{}: split into a submodule, {} file(s) moved unchanged (not counted as removed), {} new or changed",
                m.path,
                m.moved.len(),
                m.changed.len()
            ),
            "bump" => format!(
                "{}: submodule pin moved, {} file(s) changed inside, read as one change",
                m.path,
                m.changed.len()
            ),
            _ => format!("{}: submodule added, {} file(s)", m.path, m.changed.len()),
        })
        .collect()
}
