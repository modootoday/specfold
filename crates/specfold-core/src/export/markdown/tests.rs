//! Unit tests for the Markdown export.

use super::rewrite::{github_slug, strip_anchors};
use super::tree::safe_name;
use crate::paths::relative_from_file;

#[test]
fn names_and_paths() {
    assert_eq!(safe_name("a:b/c"), "a-b-c");
    assert_eq!(relative_from_file("shop/adr/a.md", "shop/sot/b.md"), "../sot/b.md");
    assert_eq!(relative_from_file("shop/adr/a.md", "shop/adr/b.md"), "b.md");
    assert_eq!(github_slug("Cart Totals (v2)"), "cart-totals-v2");
    assert_eq!(strip_anchors("## Totals {#CO-1}\n```\n## x {#y}\n```\n"), "## Totals\n```\n## x {#y}\n```\n");
}
