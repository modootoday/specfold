//! What every command returns: a schema name, an exit code, summary fields, items, and the next
//! command to run. Rendering to text or JSON is separate.

use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Code {
    /// Nothing to report.
    Ok = 0,
    /// Findings, or an ambiguous answer.
    Findings = 1,
    /// Wrong usage: a missing or bad argument.
    Usage = 2,
    /// The environment: no repository, a store or plugin that failed.
    Env = 3,
    /// A write flag was given but a safeguard refused it: nothing was written (CLI-17).
    Refused = 4,
}

#[derive(Debug, Clone)]
pub struct Item {
    pub json: Value,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct Output {
    /// `spec.<command>.v1`.
    pub schema: String,
    pub code: Code,
    /// Fields of the JSON object besides items.
    pub summary: Map<String, Value>,
    /// Lines printed before the items in text mode.
    pub head: Vec<String>,
    pub items: Vec<Item>,
    /// Whether --limit/--offset apply to the items.
    pub paged: bool,
    /// Lines printed after the items in text mode.
    pub foot: Vec<String>,
    pub next: Option<String>,
    /// A line printed after `next:` in text mode, so it is the last thing a reader sees.
    pub last: Option<String>,
}

impl Output {
    pub fn new(command: &str) -> Self {
        Output {
            schema: format!("spec.{command}.v1"),
            code: Code::Ok,
            summary: Map::new(),
            head: vec![],
            items: vec![],
            paged: true,
            foot: vec![],
            next: None,
            last: None,
        }
    }

    pub fn field(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.summary.insert(key.to_string(), value.into());
        self
    }

    pub fn head(mut self, line: impl Into<String>) -> Self {
        self.head.push(line.into());
        self
    }

    pub fn foot(mut self, line: impl Into<String>) -> Self {
        self.foot.push(line.into());
        self
    }

    pub fn item(mut self, json: Value, text: impl Into<String>) -> Self {
        self.items.push(Item { json, text: text.into() });
        self
    }

    pub fn next(mut self, cmd: impl Into<String>) -> Self {
        self.next = Some(cmd.into());
        self
    }

    /// Marks a preview: JSON gets `written: false` and `preview: true`, and text ends with a
    /// NOT WRITTEN line after `next:` so a script or agent reading only the tail cannot miss it.
    /// The exit code stays 0: a preview is a successful run.
    pub fn preview(mut self) -> Self {
        self.summary.insert("written".into(), Value::Bool(false));
        self.summary.insert("preview".into(), Value::Bool(true));
        self.last = Some("NOT WRITTEN: preview only; rerun with --write to apply".into());
        self
    }

    pub fn code(mut self, code: Code) -> Self {
        self.code = code;
        self
    }

    pub fn unpaged(mut self) -> Self {
        self.paged = false;
        self
    }

    /// An error as output: the message, and a suggestion when there is one.
    pub fn error(command: &str, code: Code, message: &str, next: Option<String>) -> Self {
        let mut o =
            Output::new(command).code(code).field("error", message.to_string()).head(format!("error: {message}"));
        o.next = next;
        o
    }
}
