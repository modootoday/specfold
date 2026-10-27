//! Arguments of the commands that check, guard and audit.

use clap::Args;

#[derive(Debug, Clone, Args)]
#[command(
    after_help = "Examples:\n  spec check --staged          # pre-commit\n  spec check --changed origin/main\n  spec check --rule L1 --rule L2 --json"
)]
pub struct CheckArgs {
    /// Only what the index changes (pre-commit).
    #[arg(long, conflicts_with = "changed")]
    pub staged: bool,
    /// Compare against this revision (pre-push).
    #[arg(long, value_name = "BASE")]
    pub changed: Option<String>,
    /// Only these rules (repeatable).
    #[arg(long = "rule", id = "rule")]
    pub rules: Vec<String>,
    /// Skip these rules (repeatable); `x` skips every plugin rule.
    #[arg(long = "skip", id = "skip")]
    pub skip: Vec<String>,
    /// Also run sot verify and generated_by commands.
    #[arg(long)]
    pub run_commands: bool,
    /// Also check citations in external roots.
    #[arg(long)]
    pub external: bool,
    /// Ask the attachment store whether stored files exist.
    #[arg(long)]
    pub online: bool,
}

#[derive(Debug, Clone, Args)]
#[command(after_help = "Examples:\n  spec verify sot:shop/cart\n  spec verify --changed origin/main")]
pub struct VerifyArgs {
    /// One source of truth; without it, all (or those whose code changed).
    pub doc: Option<String>,
    /// Only sources of truth whose governed code changed since this revision.
    #[arg(long, value_name = "BASE")]
    pub changed: Option<String>,
}

#[derive(Debug, Clone, Args)]
#[command(
    after_help = "Examples:\n  spec guard                       # every guard, each with its own range\n  spec guard public-surface --range origin/main..HEAD\n  spec guard --staged"
)]
pub struct GuardArgs {
    /// Guards to run; all when empty.
    pub names: Vec<String>,
    /// Scan the index.
    #[arg(long, conflicts_with_all = ["range", "all"])]
    pub staged: bool,
    /// A git range, or `push` for upstream..HEAD.
    #[arg(long)]
    pub range: Option<String>,
    /// Scan the whole working tree.
    #[arg(long)]
    pub all: bool,
}

#[derive(Debug, Clone, Args)]
#[command(
    after_help = "Examples:\n  spec audit --owner shop\n  spec audit --kind stale --kind gap --json\n  spec audit --pack --budget 60000\n  spec audit --record --title \"Quarterly audit\" --write\n  spec audit record --from review.md --title \"Contradiction review\" --write"
)]
pub struct AuditArgs {
    /// `record` to store a review written elsewhere.
    pub action: Option<String>,
    /// Only these owners (repeatable).
    #[arg(long = "owner", id = "owner")]
    pub owners: Vec<String>,
    /// stale, orphan, gap, stalled, open-item, debt, plugin (repeatable).
    #[arg(long = "kind", id = "kind")]
    pub kinds: Vec<String>,
    /// Build review packs for contradictions and stale statements.
    #[arg(long)]
    pub pack: bool,
    /// Bytes of documents per pack.
    #[arg(long, default_value_t = 60_000)]
    pub budget: u64,
    /// Store the diagnosis as a sealed audit document.
    #[arg(long)]
    pub record: bool,
    /// A review to store with `record`.
    #[arg(long)]
    pub from: Option<std::path::PathBuf>,
    /// Title of the audit record.
    #[arg(long)]
    pub title: Option<String>,
    /// Write the record (otherwise a preview).
    #[arg(long)]
    pub write: bool,
}

#[derive(Debug, Clone, Args)]
pub struct CensusArgs {
    /// Only report whether retired layouts are gone (exit 1 if not).
    #[arg(long)]
    pub legacy: bool,
}
