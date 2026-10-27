//! MCP tools generated from the command tree, and turning a tool call back into a command line.

use crate::cli::describe::{self, CommandInfo};
use serde_json::{Map, Value, json};

/// Commands that make no sense as tools.
const EXCLUDED: &[&str] = &["mcp", "help"];

pub fn list() -> Value {
    let tools: Vec<Value> =
        describe::commands().into_iter().filter(|c| !EXCLUDED.contains(&c.name.as_str())).map(|c| tool(&c)).collect();
    json!({ "tools": tools })
}

fn tool(c: &CommandInfo) -> Value {
    let mut props = Map::new();
    let mut required = vec![];
    for a in &c.args {
        let schema = match a.kind.as_str() {
            "bool" => json!({"type": "boolean", "description": a.help}),
            "string[]" => json!({"type": "array", "items": {"type": "string"}, "description": a.help}),
            _ => json!({"type": "string", "description": a.help}),
        };
        props.insert(a.name.clone(), schema);
        if a.required {
            required.push(a.name.clone());
        }
    }
    props.insert("limit".into(), json!({"type": "integer", "description": "items per page (default 30)"}));
    props.insert("offset".into(), json!({"type": "integer", "description": "items to skip"}));
    let mut description = c.about.clone();
    if !c.examples.is_empty() {
        description.push_str(&format!("\nExamples: {}", c.examples.join(" ; ")));
    }
    json!({
        "name": format!("spec_{}", c.name),
        "description": description,
        "inputSchema": {"type": "object", "properties": props, "required": required, "additionalProperties": false},
    })
}

fn scalar(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// `spec_find` + `{"words": ["cart"], "type": ["adr"]}` → `spec find cart --type adr --json`.
pub fn argv(name: &str, args: &Map<String, Value>) -> Result<Vec<String>, String> {
    let cmd = name.strip_prefix("spec_").ok_or_else(|| format!("unknown tool {name}"))?;
    let info = describe::commands()
        .into_iter()
        .find(|c| c.name == cmd && !EXCLUDED.contains(&c.name.as_str()))
        .ok_or_else(|| format!("unknown tool {name}"))?;
    let mut out = vec!["spec".to_string(), cmd.to_string()];
    for a in &info.args {
        let Some(v) = args.get(&a.name) else { continue };
        let values: Vec<String> = match v {
            Value::Array(items) => items.iter().map(scalar).collect(),
            Value::Bool(false) | Value::Null => continue,
            Value::Bool(true) => vec![],
            other => vec![scalar(other)],
        };
        match &a.long {
            None => out.extend(values),
            Some(long) if values.is_empty() => out.push(format!("--{long}")),
            Some(long) => {
                for val in values {
                    out.push(format!("--{long}"));
                    out.push(val);
                }
            }
        }
    }
    for k in ["limit", "offset"] {
        if let Some(v) = args.get(k) {
            out.push(format!("--{k}"));
            out.push(scalar(v));
        }
    }
    let unknown: Vec<&String> =
        args.keys().filter(|k| !info.args.iter().any(|a| &a.name == *k) && *k != "limit" && *k != "offset").collect();
    if !unknown.is_empty() {
        return Err(format!(
            "unknown argument(s) for {name}: {}",
            unknown.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
        ));
    }
    out.push("--json".into());
    Ok(out)
}
