//! What a path inside a spec directory is: a document's main file, a chapter, an attachment, or
//! something that does not belong there.

use crate::model::doctype::KeyKind;
use crate::model::registry::Registry;
use crate::model::stamp;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    Main,
    Chapter,
    Attachment,
}

/// A path that belongs to a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placed {
    pub ty: String,
    pub key: String,
    pub slug: String,
    /// The file name stem or bundle directory name: `<key>-<slug>` or `<slug>`.
    pub stem: String,
    pub bundle: bool,
    pub role: Role,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Location {
    Doc(Placed),
    /// Not a valid place for anything; the reason says why.
    Stray(String),
}

/// Splits a document stem into key and slug according to the type's key kind.
pub fn split_stem(kind: KeyKind, stem: &str) -> Result<(String, String), String> {
    match kind {
        KeyKind::Stamp => {
            let (key, slug) = stem.split_once('-').unwrap_or((stem, ""));
            if !stamp::is_stamp(key) {
                return Err(format!("name must start with a UTC timestamp YYYYMMDDHHMMSS, got '{stem}'"));
            }
            if !stamp::is_slug(slug) {
                return Err(format!("'{slug}' after the timestamp is not a kebab-case slug"));
            }
            Ok((key.to_string(), slug.to_string()))
        }
        KeyKind::Slug => {
            if !stamp::is_slug(stem) {
                return Err(format!("'{stem}' is not a kebab-case slug"));
            }
            Ok((stem.to_string(), stem.to_string()))
        }
    }
}

/// `rest` is the path below a spec directory, e.g. `adr/20260101000000-x.adr.md`.
pub fn locate(registry: &Registry, rest: &str) -> Location {
    let parts: Vec<&str> = rest.split('/').collect();
    let Some(ty) = registry.get(parts[0]) else {
        return Location::Stray(format!("'{}' is not a document type directory", parts[0]));
    };
    if parts.len() < 2 {
        return Location::Stray("a file directly in a spec directory".into());
    }
    let suffix = format!(".{}.md", ty.name);
    if parts.len() == 2 {
        let name = parts[1];
        let Some(stem) = name.strip_suffix(&suffix) else {
            let why = if name.ends_with(".md") {
                format!("a Markdown file without the .{}.md suffix", ty.name)
            } else {
                "a file outside a document bundle".to_string()
            };
            return Location::Stray(why);
        };
        if ty.form == crate::model::doctype::Form::Dir {
            return Location::Stray(format!("{} documents are directories with index{suffix}", ty.name));
        }
        return match split_stem(ty.key, stem) {
            Ok((key, slug)) => Location::Doc(Placed {
                ty: ty.name.clone(),
                key,
                slug,
                stem: stem.into(),
                bundle: false,
                role: Role::Main,
            }),
            Err(e) => Location::Stray(e),
        };
    }
    let stem = parts[1];
    let (key, slug) = match split_stem(ty.key, stem) {
        Ok(v) => v,
        Err(e) => return Location::Stray(e),
    };
    let inner = &parts[2..];
    let role = if inner.len() == 1 && inner[0] == format!("index{suffix}") {
        Role::Main
    } else if inner.len() == 1 && inner[0].ends_with(".md") {
        Role::Chapter
    } else {
        Role::Attachment
    };
    Location::Doc(Placed { ty: ty.name.clone(), key, slug, stem: stem.into(), bundle: true, role })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn reg() -> Registry {
        Registry::new(&BTreeMap::new())
    }

    #[test]
    fn places_files() {
        let r = reg();
        let Location::Doc(p) = locate(&r, "adr/20260101000000-pick-db.adr.md") else { panic!() };
        assert_eq!((p.key.as_str(), p.slug.as_str(), p.role), ("20260101000000", "pick-db", Role::Main));
        let Location::Doc(p) = locate(&r, "sot/checkout.sot.md") else { panic!() };
        assert_eq!(p.key, "checkout");
        let Location::Doc(p) = locate(&r, "plan/20260101000000-cart/10-api.md") else { panic!() };
        assert_eq!(p.role, Role::Chapter);
        let Location::Doc(p) = locate(&r, "design/20260101000000-cart/assets/a.webp") else { panic!() };
        assert_eq!(p.role, Role::Attachment);
        assert!(matches!(locate(&r, "plans/x.md"), Location::Stray(_)));
        assert!(matches!(locate(&r, "adr/ADR-001-x.adr.md"), Location::Stray(_)));
        assert!(matches!(locate(&r, "evidence/20260101000000-x.evidence.md"), Location::Stray(_)));
        assert!(matches!(locate(&r, "sot/notes.md"), Location::Stray(_)));
    }
}
