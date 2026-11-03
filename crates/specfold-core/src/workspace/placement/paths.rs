//! Where a document's files go for a given owner, type and name.

use crate::config::Config;
use crate::model::Tier;
use crate::model::doctype::{Form, KeyKind, TypeDef};
use crate::workspace::{Owner, Owners};

/// The stem of a document: `<key>-<slug>` for timestamp and number types, `<slug>` for slug types.
pub fn stem(ty: &TypeDef, key: &str, slug: &str) -> String {
    match ty.key {
        KeyKind::Stamp | KeyKind::Number => format!("{key}-{slug}"),
        KeyKind::Slug => slug.to_string(),
    }
}

/// The type directory of `owner`, relative to the repository root: the type's own directory
/// (`[types.<name>] dir`) when it has one, else its subdirectory of the spec directory. The
/// local tier always uses the second form.
pub fn type_dir(cfg: &Config, owners: &Owners, owner: &Owner, tier: Tier, ty: &TypeDef) -> String {
    let own = || crate::config::type_dirs::normalize(ty.dir.as_deref()?).ok();
    match tier {
        Tier::Committed => match own() {
            Some(dir) => owners.within(owner, &dir),
            None => format!("{}/{}", owners.spec_dir(owner, cfg.spec_dir()), ty.name),
        },
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
