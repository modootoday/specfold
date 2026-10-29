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
