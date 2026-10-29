//! What a check reports: one problem, where it is, and how to fix it.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warn,
}

/// A fix an agent can apply without judgement: a command to run, or an exact replacement.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Fix {
    Cmd(String),
    Replace { path: String, line: usize, from: String, to: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub rule: String,
    pub severity: Severity,
    pub path: String,
    pub line: usize,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<Fix>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<String>,
}

impl Finding {
    pub fn new(
        rule: impl Into<String>,
        severity: Severity,
        path: impl Into<String>,
        line: usize,
        message: impl Into<String>,
    ) -> Self {
        Finding {
            rule: rule.into(),
            severity,
            path: path.into(),
            line,
            message: message.into(),
            fix: None,
            candidates: vec![],
        }
    }

    pub fn error(rule: impl Into<String>, path: impl Into<String>, line: usize, message: impl Into<String>) -> Self {
        Self::new(rule, Severity::Error, path, line, message)
    }

    pub fn warn(rule: impl Into<String>, path: impl Into<String>, line: usize, message: impl Into<String>) -> Self {
        Self::new(rule, Severity::Warn, path, line, message)
    }

    pub fn fix_cmd(mut self, cmd: impl Into<String>) -> Self {
        self.fix = Some(Fix::Cmd(cmd.into()));
        self
    }

    pub fn fix_replace(mut self, from: impl Into<String>, to: impl Into<String>) -> Self {
        self.fix = Some(Fix::Replace { path: self.path.clone(), line: self.line, from: from.into(), to: to.into() });
        self
    }

    pub fn with_candidates(mut self, c: Vec<String>) -> Self {
        self.candidates = c;
        self
    }
}

/// Text made safe to print: invisible characters and controls (zero-width, bidirectional, BOM,
/// C0/C1) are written as `\u{XXXX}`, so a terminal or an editor title shows them instead of
/// hiding them or reordering the text around them.
pub fn visible(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let hide = matches!(c, '\u{200B}' | '\u{200E}' | '\u{200F}' | '\u{2028}'..='\u{202E}' | '\u{2060}'..='\u{2069}' | '\u{FEFF}')
            || (c.is_control() && c != '\t');
        if hide {
            out.push_str(&format!("\\u{{{:04X}}}", c as u32));
        } else {
            out.push(c);
        }
    }
    out
}
