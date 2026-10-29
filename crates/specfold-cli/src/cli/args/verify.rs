//! Arguments of the commands that check, guard and audit.

use clap::Args;

#[derive(Debug, Clone, Args)]
#[command(
    after_help = "Examples:\n  spec check --staged          # pre-commit\n  spec check --changed origin/main\n  spec check --rule L1 --rule L2 --json\n  spec check --new              # which documents were just recognised\n  spec check --summary          # findings per rule\n  spec check -q                 # errors only\n  spec check --rule N1          # counts in spec:count markers"
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
    /// In a full run, also run every guard over the whole tree (staged and ranged runs always do).
    #[arg(long)]
    pub guards: bool,
    /// Only errors of these rules fail the run (repeatable); `any` means every rule.
    #[arg(long = "fail-on", value_name = "RULE")]
    pub fail_on: Vec<String>,
    /// Show every finding, ignoring [check] baseline.
    #[arg(long, conflicts_with = "write_baseline")]
    pub no_baseline: bool,
    /// Record today's errors as the baseline ([check] baseline); a full run on a clean tree only.
    #[arg(long, conflicts_with_all = ["staged", "changed"])]
    pub write_baseline: bool,
    /// Also list the documents not in the last commit (new or not yet committed).
    #[arg(long)]
    pub new: bool,
    /// Count findings per rule instead of listing them.
    #[arg(long)]
    pub summary: bool,
    /// List errors only; warnings are still counted.
    #[arg(long, short = 'q')]
    pub errors_only: bool,
    /// Findings for CI: github (workflow annotations and step summary), sarif (code scanning),
    /// gitlab (Code Quality) or rdjson (reviewdog); text is the default.
    #[arg(long, default_value = "text", value_parser = ["text", "github", "sarif", "gitlab", "rdjson"])]
    pub format: String,
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
    /// Record each guard's current hits in its baseline file; a clean tree only.
    #[arg(long, conflicts_with_all = ["staged", "range", "all"])]
    pub write_baseline: bool,
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
    /// stale, orphan, gap, stalled, finished, open-item, debt, drift, plugin (repeatable).
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
