//! The status lifecycle shared by every type: draft → active → superseded | archived, and
//! draft → rejected. No transition goes backwards.

pub const STATUSES: [&str; 5] = ["draft", "active", "superseded", "archived", "rejected"];

/// Statuses that `find` and `context` show by default.
pub fn is_current(status: &str) -> bool {
    matches!(status, "draft" | "active")
}

pub fn transition_allowed(from: &str, to: &str) -> bool {
    from == to
        || matches!(
            (from, to),
            ("draft", "active")
                | ("draft", "rejected")
                | ("draft", "archived")
                | ("active", "superseded")
                | ("active", "archived")
                | ("superseded", "archived")
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_only() {
        assert!(transition_allowed("draft", "active"));
        assert!(transition_allowed("active", "superseded"));
        assert!(!transition_allowed("active", "draft"));
        assert!(!transition_allowed("archived", "active"));
        assert!(!transition_allowed("rejected", "active"));
    }
}
