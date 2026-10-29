//! Document types and the built-in preset that covers design, planning and maintenance material.

use super::lifecycle;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyKind {
    /// A UTC timestamp `YYYYMMDDHHMMSS`, assigned when the document is created.
    Stamp,
    /// A kebab-case name chosen by the author.
    Slug,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Form {
    /// A single file, or a directory when the document has chapters or files.
    Either,
    /// Always a directory with `index.<type>.md`.
    Dir,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeDef {
    pub name: String,
    pub about: String,
    pub key: KeyKind,
    pub form: Form,
    pub statuses: Vec<String>,
    /// Status a new document starts in.
    pub initial: String,
    /// Once created, the body may not change.
    #[serde(default)]
    pub sealed: bool,
    /// Extra frontmatter fields this type understands, for `spec schema`.
    #[serde(default)]
    pub fields: Vec<String>,
    /// The body skeleton `spec new` writes.
    #[serde(default)]
    pub template: String,
    /// Allowed status moves, `from → [to]`; empty means the shared lifecycle.
    #[serde(default)]
    pub transitions: BTreeMap<String, Vec<String>>,
    /// Frontmatter keys a status requires, e.g. a review date for an accepted risk.
    #[serde(default)]
    pub require_when: BTreeMap<String, Vec<String>>,
}

impl TypeDef {
    pub fn transition_allowed(&self, from: &str, to: &str) -> bool {
        if from == to {
            return true;
        }
        if self.transitions.is_empty() {
            return lifecycle::transition_allowed(from, to);
        }
        self.transitions.get(from).is_some_and(|next| next.iter().any(|s| s == to))
    }
}

pub struct Spec<'a> {
    pub name: &'a str,
    pub about: &'a str,
    pub key: KeyKind,
    pub form: Form,
    pub statuses: &'a [&'a str],
    pub initial: &'a str,
    pub sealed: bool,
    pub fields: &'a [&'a str],
    pub template: &'a str,
}

impl From<Spec<'_>> for TypeDef {
    fn from(s: Spec<'_>) -> Self {
        TypeDef {
            name: s.name.into(),
            about: s.about.into(),
            key: s.key,
            form: s.form,
            statuses: s.statuses.iter().map(|x| x.to_string()).collect(),
            initial: s.initial.into(),
            sealed: s.sealed,
            fields: s.fields.iter().map(|x| x.to_string()).collect(),
            template: s.template.into(),
            transitions: BTreeMap::new(),
            require_when: BTreeMap::new(),
        }
    }
}

const LIFECYCLE: &[&str] = &["draft", "active", "superseded", "archived"];
const RECORD: &[&str] = &["active", "superseded", "archived"];

pub fn builtin() -> Vec<TypeDef> {
    use Form::*;
    use KeyKind::*;
    let specs = [
        Spec {
            name: "sot",
            about: "A source of truth: the rule that holds now.",
            key: Slug,
            form: Either,
            statuses: LIFECYCLE,
            initial: "draft",
            sealed: false,
            fields: &["verify", "verify_timeout_ms", "applies_to"],
            template: "## Rule\n\n## Why\n\n## Verification\n",
        },
        Spec {
            name: "adr",
            about: "A decision and the reason for it.",
            key: Stamp,
            form: Either,
            statuses: &["draft", "active", "superseded", "rejected"],
            initial: "draft",
            sealed: false,
            fields: &["decided_because"],
            template: "## Context\n\n## Decision\n\n## Consequences\n",
        },
        Spec {
            name: "plan",
            about: "A plan of work and its running record.",
            key: Stamp,
            form: Either,
            statuses: LIFECYCLE,
            initial: "draft",
            sealed: false,
            fields: &["phase"],
            template: "## Goal\n\n## Steps\n\n## Record\n",
        },
        Spec {
            name: "research",
            about: "A study, benchmark or evaluation and what it found.",
            key: Stamp,
            form: Either,
            statuses: RECORD,
            initial: "active",
            sealed: false,
            fields: &[],
            template: "## Question\n\n## Method\n\n## Findings\n",
        },
        Spec {
            name: "evidence",
            about: "The cover of a bundle of measurements or raw outputs.",
            key: Stamp,
            form: Dir,
            statuses: &["active", "archived"],
            initial: "active",
            sealed: false,
            fields: &[],
            template: "## What was measured\n\n## How to read it\n",
        },
        Spec {
            name: "audit",
            about: "A security or compliance audit snapshot. Its body is sealed once created.",
            key: Stamp,
            form: Either,
            statuses: &["active", "superseded"],
            initial: "active",
            sealed: true,
            fields: &[],
            template: "## Scope\n\n## Method\n\n## Findings\n",
        },
        Spec {
            name: "page",
            about: "The specification of one screen or page.",
            key: Slug,
            form: Either,
            statuses: LIFECYCLE,
            initial: "draft",
            sealed: false,
            fields: &[],
            template: "## Purpose\n\n## Layout\n\n## States\n",
        },
        Spec {
            name: "design",
            about: "A bundle of design boards: the layout of record for the screens it shows.",
            key: Stamp,
            form: Dir,
            statuses: LIFECYCLE,
            initial: "draft",
            sealed: false,
            fields: &["screens"],
            template: "## Screens\n\n## Notes\n",
        },
        Spec {
            name: "guide",
            about: "How to develop, operate or use something.",
            key: Slug,
            form: Either,
            statuses: RECORD,
            initial: "active",
            sealed: false,
            fields: &[],
            template: "## Overview\n\n## Steps\n",
        },
        Spec {
            name: "runbook",
            about: "A step-by-step operational procedure: incidents, cutovers, deploys.",
            key: Slug,
            form: Either,
            statuses: RECORD,
            initial: "active",
            sealed: false,
            fields: &["last_run"],
            template: "## When to use\n\n## Steps\n\n## Rollback\n",
        },
        Spec {
            name: "change",
            about: "One user-facing change for the next release notes; active until released, then archived with release set.",
            key: Stamp,
            form: Either,
            statuses: RECORD,
            initial: "active",
            sealed: false,
            fields: &["kind", "release"],
            template: "## Change\n\n## Migration\n",
        },
    ];
    specs.into_iter().map(TypeDef::from).collect()
}

/// A type a project adds without overriding every field.
pub fn custom(name: &str) -> TypeDef {
    Spec {
        name,
        about: "",
        key: KeyKind::Stamp,
        form: Form::Either,
        statuses: LIFECYCLE,
        initial: "draft",
        sealed: false,
        fields: &[],
        template: "",
    }
    .into()
}
