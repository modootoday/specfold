//! Document identifiers: `<type>:<owner>/<key>`, optionally followed by `#<anchor>`.

use serde::{Serialize, Serializer};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DocId {
    pub ty: String,
    pub owner: String,
    pub key: String,
}

impl DocId {
    pub fn new(ty: &str, owner: &str, key: &str) -> Self {
        DocId { ty: ty.into(), owner: owner.into(), key: key.into() }
    }

    /// Parses `type:owner/key`. The owner may contain dots; the key may not contain `/`.
    pub fn parse(s: &str) -> Option<Self> {
        let (ty, rest) = s.split_once(':')?;
        let (owner, key) = rest.rsplit_once('/')?;
        let valid = |p: &str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_alphanumeric() || b"-._".contains(&b));
        if !valid(ty) || !valid(owner) || !valid(key) {
            return None;
        }
        Some(DocId::new(ty, owner, key))
    }
}

impl fmt::Display for DocId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}/{}", self.ty, self.owner, self.key)
    }
}

impl Serialize for DocId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

/// A reference as written in a document or on the command line: an id, an id with an anchor,
/// an alias, or a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefText {
    pub target: String,
    pub anchor: Option<String>,
}

impl RefText {
    pub fn parse(s: &str) -> Self {
        let s = s.trim();
        match s.split_once('#') {
            Some((t, a)) if !a.is_empty() => RefText { target: t.to_string(), anchor: Some(a.to_string()) },
            _ => RefText { target: s.trim_end_matches('#').to_string(), anchor: None },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ids() {
        let id = DocId::parse("adr:core.infra.proxy/20261027073000").unwrap();
        assert_eq!(id.owner, "core.infra.proxy");
        assert_eq!(id.to_string(), "adr:core.infra.proxy/20261027073000");
        assert!(DocId::parse("ADR-7").is_none());
        assert!(DocId::parse("plan:/x").is_none());
        let r = RefText::parse("sot:shop/checkout#CO-102");
        assert_eq!(r.anchor.as_deref(), Some("CO-102"));
    }
}
