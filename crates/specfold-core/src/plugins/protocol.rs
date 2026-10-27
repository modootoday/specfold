//! The `specfold.plugin.v1` messages. One request per process: specfold writes a JSON object to
//! the plugin's stdin and reads one JSON object from its stdout.

use crate::check::finding::{Fix, Severity};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PROTOCOL: &str = "specfold.plugin.v1";

#[derive(Debug, Clone, Serialize)]
pub struct Request<'a> {
    pub protocol: &'static str,
    pub method: &'a str,
    #[serde(skip_serializing_if = "Value::is_null")]
    pub params: Value,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct PluginRule {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub explain: String,
    #[serde(default)]
    pub on_commit: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Provides {
    #[serde(default)]
    pub rules: Vec<PluginRule>,
    #[serde(default)]
    pub importers: Vec<String>,
    #[serde(default)]
    pub exporters: Vec<String>,
    #[serde(default)]
    pub audits: Vec<String>,
    #[serde(default)]
    pub events: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Description {
    #[serde(default)]
    pub protocol: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub provides: Provides,
}

/// A finding as a plugin reports it; the rule id is namespaced by specfold.
#[derive(Debug, Clone, Deserialize)]
pub struct PluginFinding {
    pub rule: String,
    #[serde(default = "default_severity")]
    pub severity: Severity,
    pub path: String,
    #[serde(default = "one")]
    pub line: usize,
    pub message: String,
    pub fix: Option<Fix>,
    #[serde(default)]
    pub candidates: Vec<String>,
}

fn default_severity() -> Severity {
    Severity::Error
}

fn one() -> usize {
    1
}

#[derive(Debug, Clone, Deserialize)]
pub struct FindingsResponse {
    #[serde(default)]
    pub findings: Vec<PluginFinding>,
}
