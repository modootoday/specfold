//! Owners: the units that own documents (the repository root, a product, a package).

use super::patterns::PathMatch;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Owner {
    pub id: String,
    /// Directory relative to the repository root, "" for the root owner.
    pub path: String,
    /// The kind pattern that produced it, "" for the root owner.
    pub kind: String,
    pub public: bool,
    /// For a public owner: the directory its documents go to instead.
    pub companion: Option<String>,
    pub retired: bool,
}

#[derive(Debug, Clone)]
pub struct Owners {
    /// The root owner first, then every discovered owner.
    pub list: Vec<Owner>,
    pub(super) public: Vec<PathMatch>,
}
