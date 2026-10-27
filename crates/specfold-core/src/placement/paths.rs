//! Where a document's files go for a given owner, type and name.

use crate::config::Config;
use crate::model::Tier;
use crate::model::doctype::{Form, KeyKind, TypeDef};
use crate::workspace::{Owner, Owners};

/// The stem of a document: `<key>-<slug>` for timestamp types, `<slug>` for slug types.
pub fn stem(ty: &TypeDef, key: &str, slug: &str) -> String {
    match ty.key {
        KeyKind::Stamp => format!("{key}-{slug}"),
        KeyKind::Slug => slug.to_string(),
    }
}

/// The type directory of `owner`, relative to the repository root.
pub fn type_dir(cfg: &Config, owners: &Owners, owner: &Owner, tier: Tier, ty: &TypeDef) -> String {
    match tier {
        Tier::Committed => format!("{}/{}", owners.spec_dir(owner, cfg.spec_dir()), ty.name),
        Tier::Local => format!("{}/{}/{}", cfg.local_dir(), owner.id, ty.name),
    }
}

/// The main file of a document. Directory-form types, and documents that need a bundle, get
/// `<stem>/index.<type>.md`.
pub fn main_file(type_dir: &str, ty: &TypeDef, stem: &str, bundle: bool) -> String {
    if bundle || ty.form == Form::Dir {
        format!("{type_dir}/{stem}/index.{}.md", ty.name)
    } else {
        format!("{type_dir}/{stem}.{}.md", ty.name)
    }
}
