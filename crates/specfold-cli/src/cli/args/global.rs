//! Flags every command takes.

use clap::Args;
use std::path::PathBuf;

#[derive(Debug, Clone, Args)]
pub struct Global {
    /// Print one JSON object (schema `spec.<command>.v1`).
    #[arg(long, global = true)]
    pub json: bool,
    /// Print one JSON object per item, for streaming.
    #[arg(long, global = true, conflicts_with = "json")]
    pub jsonl: bool,
    /// Items per page for list output.
    #[arg(long, global = true, default_value_t = 30, value_parser = positive)]
    pub limit: usize,
    /// Size budget in bytes for the items of list output: whole items are kept while they fit
    /// (at least one), the rest are left for --offset. Each item counts as rendered.
    #[arg(long, global = true, value_name = "BYTES")]
    pub max_bytes: Option<usize>,
    /// Items to skip for list output.
    #[arg(long, global = true, default_value_t = 0)]
    pub offset: usize,
    /// Run as if started in this directory.
    #[arg(short = 'C', long = "dir", global = true, value_name = "DIR")]
    pub dir: Option<PathBuf>,
    /// Leave out the git-ignored local documents.
    #[arg(long, global = true)]
    pub no_local: bool,
}

/// C152: `--limit 0` would show nothing and point at itself; a page holds at least one item.
fn positive(s: &str) -> Result<usize, String> {
    match s.parse::<usize>() {
        Ok(0) => Err("must be at least 1".into()),
        Ok(n) => Ok(n),
        Err(e) => Err(e.to_string()),
    }
}
