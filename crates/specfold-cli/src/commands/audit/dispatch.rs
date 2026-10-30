//! Picks the audit action from the arguments.

use super::{diagnose, pack, record};
use crate::cli::args::{global::Global, verify::AuditArgs};
use crate::output::Output;
use crate::session;
use anyhow::{Result, bail};

pub fn run(g: &Global, a: &AuditArgs) -> Result<Output> {
    let s = session::open(g)?;
    match a.action.as_deref() {
        Some("record") => {
            let Some(from) = &a.from else { bail!("audit record needs --from <file> with the review") };
            record::run(&s, a, std::fs::read_to_string(from)?)
        }
        Some(other) => bail!("unknown audit action '{other}' (record)"),
        None if a.pack => pack::run(&s, a),
        None => {
            let diag = diagnose::diagnose(&s, a)?;
            if a.record { record::run(&s, a, diagnose::as_markdown(&diag)) } else { diagnose::render(&diag) }
        }
    }
}
