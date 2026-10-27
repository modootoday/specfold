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
    #[arg(long, global = true, default_value_t = 30)]
    pub limit: usize,
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
