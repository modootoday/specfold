//! `spec schema`: document types and their frontmatter as JSON Schema, or output schema names.

use crate::cli::args::{global::Global, setup::SchemaArgs};
use crate::cli::describe;
use crate::output::Output;
use crate::session;
use anyhow::{Result, bail};
use serde_json::{Value, json};
use specfold_core::model::TypeDef;

fn frontmatter_schema(t: &TypeDef) -> Value {
    let mut props = serde_json::Map::new();
    props.insert(
        "id".into(),
        json!({"type": "string", "pattern": format!("^{}:[A-Za-z0-9._-]+/[A-Za-z0-9._-]+$", t.name)}),
    );
    props.insert("title".into(), json!({"type": "string", "minLength": 1}));
    props.insert("status".into(), json!({"enum": t.statuses}));
    props.insert("created".into(), json!({"type": "string", "anyOf": [{"format": "date"}, {"format": "date-time"}]}));
    props.insert("updated".into(), json!({"type": "string", "anyOf": [{"format": "date"}, {"format": "date-time"}]}));
    for k in ["owners", "tags", "aliases", "check_skip"] {
        props.insert(k.into(), json!({"type": "array", "items": {"type": "string"}}));
    }
    props.insert(
        "rel".into(),
        json!({"type": "object", "properties": {
        "governs": {"type": "array", "items": {"type": "string"}, "description": "code paths or globs"},
        "supersedes": {"type": "array", "items": {"type": "string"}},
        "implements": {"type": "array", "items": {"type": "string"}},
        "evidence": {"type": "array", "items": {"type": "string"}},
        "depends_on": {"type": "array", "items": {"type": "string"}}
    }, "additionalProperties": false}),
    );
    props.insert("files".into(), json!({"type": "array", "items": {"type": "object", "required": ["path", "sha256", "bytes", "store"], "properties": {
        "path": {"type": "string"}, "sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"}, "bytes": {"type": "integer"}, "store": {"enum": ["repo", "store"]}
    }}}));
    props.insert("generated_by".into(), json!({"type": "string"}));
    for f in &t.fields {
        props.entry(f.clone()).or_insert(json!({}));
    }
    json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "title": format!("{} frontmatter", t.name), "type": "object", "required": ["id", "title", "status", "created", "updated"], "properties": props})
}

pub fn run(g: &Global, a: &SchemaArgs) -> Result<Output> {
    let cfg = session::config(g)?;
    let mut o = Output::new("schema");
    match a.ty.as_deref() {
        Some("output") => {
            for c in describe::commands() {
                o = o.item(json!({"command": c.name, "schema": c.output}), format!("{:<9} {}", c.name, c.output));
            }
            Ok(o)
        }
        Some(name) => {
            let Some(t) = cfg.registry.get(name) else {
                bail!("unknown type '{name}' (have: {})", cfg.registry.names().join(", "))
            };
            Ok(o.unpaged()
                .field("type", serde_json::to_value(t)?)
                .field("frontmatter", frontmatter_schema(t))
                .head(format!("{}: {}", t.name, t.about))
                .next(format!("spec new {} --title '…'", t.name)))
        }
        None => {
            for t in &cfg.registry.types {
                o = o.item(
                    json!({"name": t.name, "about": t.about, "key": t.key, "statuses": t.statuses}),
                    format!("{:<9} {}", t.name, t.about),
                );
            }
            Ok(o.next("spec schema <type>"))
        }
    }
}
