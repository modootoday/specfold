//! The type registry: the built-in preset with a project's overrides applied.

use super::doctype::{self, Form, KeyKind, TypeDef};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Overrides for one type from spec.toml.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TypeOverride {
    pub enabled: Option<bool>,
    pub about: Option<String>,
    pub key: Option<KeyKind>,
    pub form: Option<Form>,
    pub statuses: Option<Vec<String>>,
    pub initial: Option<String>,
    pub sealed: Option<bool>,
    pub fields: Option<Vec<String>>,
    pub template: Option<String>,
    pub transitions: Option<BTreeMap<String, Vec<String>>>,
    pub require_when: Option<BTreeMap<String, Vec<String>>>,
    pub sections: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct Registry {
    pub types: Vec<TypeDef>,
}

fn apply(t: &mut TypeDef, o: &TypeOverride) {
    if let Some(v) = &o.about {
        t.about = v.clone();
    }
    if let Some(v) = o.key {
        t.key = v;
    }
    if let Some(v) = o.form {
        t.form = v;
    }
    if let Some(v) = &o.statuses {
        t.statuses = v.clone();
    }
    if let Some(v) = &o.initial {
        t.initial = v.clone();
    }
    if let Some(v) = o.sealed {
        t.sealed = v;
    }
    if let Some(v) = &o.fields {
        t.fields = v.clone();
    }
    if let Some(v) = &o.template {
        t.template = v.clone();
    }
    if let Some(v) = &o.transitions {
        t.transitions = v.clone();
    }
    if let Some(v) = &o.require_when {
        t.require_when = v.clone();
    }
    if let Some(v) = &o.sections {
        t.sections = v.clone();
    }
}

/// `[types.<name>]` tables that look like typos: a name that is not built in, close to one that
/// is, and without an `about` (which a deliberate new type would have); or a table disabling a
/// type that does not exist.
pub fn suspect_overrides(overrides: &BTreeMap<String, TypeOverride>) -> Vec<String> {
    let builtin: Vec<String> = doctype::builtin().into_iter().map(|t| t.name).collect();
    let mut out = vec![];
    for (name, o) in overrides {
        if builtin.contains(name) {
            continue;
        }
        let near = crate::check::rules::support::nearest(name, builtin.iter().map(String::as_str))
            .or_else(|| builtin.iter().map(String::as_str).find(|b| name.strip_suffix('s') == Some(*b)));
        if o.enabled == Some(false) {
            let hint = near.map(|n| format!("; did you mean [types.{n}]?")).unwrap_or_default();
            out.push(format!("[types.{name}] disables a type that does not exist{hint}"));
        } else if let Some(n) = near
            && o.about.is_none()
        {
            out.push(format!(
                "[types.{name}] is not a built-in type, so it creates a new one; did you mean [types.{n}]? (give it an `about` to keep it as a new type)"
            ));
        }
    }
    out
}

impl Registry {
    pub fn new(overrides: &BTreeMap<String, TypeOverride>) -> Self {
        let mut types = doctype::builtin();
        for (name, o) in overrides {
            let pos = types.iter().position(|t| &t.name == name);
            if o.enabled == Some(false) {
                if let Some(p) = pos {
                    types.remove(p);
                }
                continue;
            }
            match pos {
                Some(p) => apply(&mut types[p], o),
                None => {
                    let mut t = doctype::custom(name);
                    apply(&mut t, o);
                    types.push(t);
                }
            }
        }
        Registry { types }
    }

    pub fn get(&self, name: &str) -> Option<&TypeDef> {
        self.types.iter().find(|t| t.name == name)
    }

    pub fn names(&self) -> Vec<&str> {
        self.types.iter().map(|t| t.name.as_str()).collect()
    }
}

#[cfg(test)]
mod suspect_tests {
    use super::{TypeOverride, suspect_overrides};
    use std::collections::BTreeMap;

    #[test]
    fn near_misses_are_named_and_deliberate_types_are_not() {
        let mut m = BTreeMap::new();
        m.insert("adrs".to_string(), TypeOverride { sections: Some(vec!["Decision".into()]), ..Default::default() });
        m.insert("memo".to_string(), TypeOverride { about: Some("Short notes".into()), ..Default::default() });
        m.insert("rfc".to_string(), TypeOverride::default());
        let got = suspect_overrides(&m);
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(got[0].contains("did you mean [types.adr]"), "{got:?}");
    }
}
