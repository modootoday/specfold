//! Refusing to write a document where `[forbid] paths` would make Z1 reject it, before anything
//! is written.

use crate::corpus::Corpus;
use crate::workspace::owners::matcher;
use anyhow::{Result, bail};

/// The [forbid] path a file or any directory above it matches.
pub fn matched(c: &Corpus, path: &str) -> Option<String> {
    let parts: Vec<&str> = path.split('/').collect();
    c.cfg.raw.forbid.paths.iter().find_map(|pat| {
        let m = matcher(pat)?;
        (1..=parts.len()).any(|n| m.is_match(&parts[..n].join("/"))).then(|| pat.clone())
    })
}

/// Fails with the forbidden pattern and `hint`, the way out for the command that asked.
pub fn refuse(c: &Corpus, path: &str, hint: &str) -> Result<()> {
    if let Some(pat) = matched(c, path) {
        bail!("{path} matches the forbidden path {pat} in spec.toml [forbid]; {hint}");
    }
    Ok(())
}

/// The way out shared by commands that cannot write to the local directory themselves.
pub const NO_LOCAL_HINT: &str = "create it with spec new --local (the git-ignored local directory), or change [forbid] paths or the type's directory";
