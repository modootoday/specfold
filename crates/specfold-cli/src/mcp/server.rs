//! A minimal MCP server over stdio: newline-delimited JSON-RPC 2.0 with initialize, ping,
//! tools/list and tools/call.

use super::tools;
use crate::execute::execute;
use crate::output::{Code, Format, render};
use serde_json::{Value, json};
use std::io::{BufRead, Write};

const PROTOCOL_VERSION: &str = "2025-06-18";

fn reply(id: &Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn fail(id: &Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

fn call(params: &Value) -> Value {
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let empty = serde_json::Map::new();
    let args = params.get("arguments").and_then(Value::as_object).unwrap_or(&empty);
    match tools::argv(name, args) {
        Ok(argv) => {
            let ex = execute(argv);
            let text = render(&ex.output, Format::Json, &ex.page);
            json!({"content": [{"type": "text", "text": text}], "isError": matches!(ex.output.code, Code::Usage | Code::Env)})
        }
        Err(e) => json!({"content": [{"type": "text", "text": e}], "isError": true}),
    }
}

pub fn handle(msg: &Value) -> Option<Value> {
    let id = msg.get("id")?.clone();
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    Some(match method {
        "initialize" => reply(
            &id,
            json!({
                "protocolVersion": params.get("protocolVersion").and_then(Value::as_str).unwrap_or(PROTOCOL_VERSION),
                "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": {"name": "specfold", "version": env!("CARGO_PKG_VERSION")},
                "instructions": "Call spec_context with the code paths you will change before editing them. Create documents with spec_new and change them with spec_set; run spec_check before committing."
            }),
        ),
        "ping" => reply(&id, json!({})),
        "tools/list" => reply(&id, tools::list()),
        "tools/call" => reply(&id, call(&params)),
        other => fail(&id, -32601, &format!("method not found: {other}")),
    })
}

pub fn serve() -> std::io::Result<()> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => handle(&msg),
            Err(e) => Some(fail(&Value::Null, -32700, &format!("parse error: {e}"))),
        };
        if let Some(r) = response {
            writeln!(stdout, "{r}")?;
            stdout.flush()?;
        }
    }
    Ok(())
}
