//! One exported page: where it goes, its title, its body and what the body links to.

use crate::export::links::Link;
use std::collections::HashMap;

/// One exported page.
pub(super) struct Page {
    pub(super) idx: usize,
    /// Path in the tree, e.g. `shop/adr/cart-totals.md`.
    pub(super) file: String,
    pub(super) title: String,
    /// Body with chapters appended, and its resolved references.
    pub(super) body: String,
    pub(super) links: Vec<Link>,
    /// Explicit anchor -> heading text.
    pub(super) headings: HashMap<String, String>,
}
