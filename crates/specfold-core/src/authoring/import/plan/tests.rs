//! Unit tests for the import plan.

#[test]
fn record_numbers_come_off_titles() {
    assert_eq!(super::names::strip_number("2. Use X"), "Use X");
    assert_eq!(super::names::strip_number("12. "), "12. ");
    assert_eq!(super::names::strip_number("2026 plan"), "2026 plan");
}

#[test]
fn sections_come_out_and_links_are_made_relative() {
    let body = "\n# T\n\n## Status\n\nAccepted\n\n## Context\n\nx\n";
    assert_eq!(super::row::without_section(body, "status").unwrap(), "\n# T\n\n## Context\n\nx\n");
    assert_eq!(crate::paths::relative_from_dir(".spec/adr", ".spec/adr/a.adr.md"), "a.adr.md");
    assert_eq!(crate::paths::relative_from_dir("svc/.spec/adr", ".spec/adr/a.md"), "../../../.spec/adr/a.md");
    assert_eq!(crate::paths::join("doc/adr", "../x/./y.md").as_deref(), Some("doc/x/y.md"));
    assert_eq!(crate::paths::join("", "../y.md"), None);
    assert_eq!(
        super::links::by_candidates("[ADR-0002](0002-use-y.md)"),
        ["0002-use-y", "0002-use-y.md", "ADR-0002", "0002"]
    );
}
