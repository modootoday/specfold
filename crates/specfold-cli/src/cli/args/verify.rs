//! Arguments of the commands that check, guard and audit.

use clap::Args;

#[derive(Debug, Clone, Args)]
#[command(
    after_help = "Examples:\n  spec check --staged          # pre-commit\n  spec check --changed origin/main\n  spec check --rule L1 --rule L2 --json\n  spec check --new              # which documents were just recognised\n  spec check --summary          # findings per rule\n  spec check -q                 # errors only\n  spec check docs/a.md --format editor  # one file, for an editor on save\n  spec check --stdin-path docs/a.md --format editor < buffer  # unsaved text\n  spec check --rule N1          # counts in spec:count markers"
)]
pub struct CheckArgs {
    /// Only report findings in these files or document ids (an editor checking the file it saved). Every
    /// document is still read so references resolve; counts and the exit code follow the files.
    #[arg(value_name = "FILE", conflicts_with = "write_baseline")]
    pub files: Vec<String>,
    /// Check this document with its text read from standard input (an unsaved editor buffer);
    /// the file on disk is left alone and only its findings are reported.
    #[arg(long, value_name = "FILE", conflicts_with = "write_baseline")]
    pub stdin_path: Option<String>,
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
    /// Only errors of these rules fail the run (repeatable); `any` means every rule, `none` reports
    /// without failing.
    #[arg(long = "fail-on", value_name = "RULE")]
    pub fail_on: Vec<String>,
    /// Show every finding, ignoring [check] baseline.
    #[arg(long, conflicts_with = "write_baseline")]
    pub no_baseline: bool,
    /// Record today's errors as the baseline ([check] baseline); a full run on a clean tree only.
    #[arg(long, conflicts_with_all = ["staged", "changed"])]
    pub write_baseline: bool,
    /// Confirm an action that deletes, overwrites or sends files out (with its write flag).
    #[arg(long)]
    pub yes: bool,
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
    /// gitlab (Code Quality) or rdjson (reviewdog); for editors: editor
    /// (`path:line:col: severity RULE: message`); text is the default.
    #[arg(long, default_value = "text", value_parser = ["text", "github", "sarif", "gitlab", "rdjson", "editor"])]
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
    /// Confirm an action that deletes, overwrites or sends files out (with its write flag).
    #[arg(long)]
    pub yes: bool,
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
    /// stale, orphan, gap, stalled, finished, open-item, exception, debt, drift, plugin (repeatable).
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
    /// Preview only, writing nothing (the default without --write).
    #[arg(long, conflicts_with = "write")]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Args)]
pub struct CensusArgs {
    /// Only report whether retired layouts are gone (exit 1 if not).
    #[arg(long)]
    pub legacy: bool,
    /// Report translations (rel.translation_of): which are behind their source, which record no
    /// source_rev, and which sources each language lacks. Exit 1 if any (see --fail-on).
    #[arg(long)]
    pub translations: bool,
    /// With --translations: the languages to cover (repeatable); default every language used.
    #[arg(long, requires = "translations")]
    pub lang: Vec<String>,
    /// With --translations: what fails the run (exit 1). `any` (default) is every finding,
    /// missing translations included; `stale` only translations behind, without source_rev or
    /// without a source; `none` reports without failing.
    #[arg(long = "fail-on", value_name = "WHAT", default_value = "any", requires = "translations",
          value_parser = ["any", "stale", "none"])]
    pub fail_on: String,
}
