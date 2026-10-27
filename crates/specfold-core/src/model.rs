//! The data model: identifiers, types, lifecycle and documents.

pub mod doc;
pub mod doctype;
pub mod id;
pub mod lifecycle;
pub mod registry;
pub mod stamp;

pub use doc::{Doc, FileEntry, Located, Tier};
pub use doctype::{Form, KeyKind, TypeDef};
pub use id::{DocId, RefText};
pub use registry::Registry;
