//! Arguments of the commands that write.

use clap::Args;
use std::path::PathBuf;

#[derive(Debug, Clone, Args)]
#[command(
    after_help = "Examples:\n  spec new adr --governs services/shop/src/db --title \"Use Postgres for orders\"\n  spec new adr --from adr:shop/20250101000000 --title \"Move orders to a queue\"\n  spec new plan --governs services/shop --title \"Checkout v2\" --local\n\nTimestamp keys and the created/updated dates are UTC."
)]
pub struct NewArgs {
    /// The document type (spec schema lists them).
    #[arg(id = "type", value_name = "TYPE")]
    pub ty: String,
    /// One-line title; also the source of the slug. Also accepted as the second argument.
    #[arg(long, required_unless_present = "title_text")]
    pub title: Option<String>,
    /// The title, as `spec new adr "Use files"`.
    #[arg(id = "title_text", value_name = "TITLE", conflicts_with = "title")]
    pub title_text: Option<String>,
    /// Code this document rules; decides where it is stored.
    #[arg(long)]
    pub governs: Vec<String>,
    /// Store with this owner instead of the computed one.
    #[arg(long)]
    pub owner: Option<String>,
    /// For slug-keyed types: the slug (defaults to the title).
    #[arg(long)]
    pub slug: Option<String>,
    /// Supersede this document; its status becomes superseded.
    #[arg(long)]
    pub from: Option<String>,
    /// Create in the git-ignored local directory.
    #[arg(long)]
    pub local: bool,
    /// Create a bundle directory even for a single-file type.
    #[arg(long)]
    pub bundle: bool,
    /// For skill, rule and agent: the description (defaults to the title).
    #[arg(long)]
    pub description: Option<String>,
    /// Show what would be created without writing.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Args)]
#[command(
    after_help = "Examples:\n  spec set adr:shop/20260101000000 status=active\n  spec set plan:shop/20260101000000 phase=P3 rel.implements+=sot:shop/cart\n  spec set sot:shop/cart tags-=draft\n  spec set sot:shop/cart verified_at=HEAD   # pin: audit flags any later commit to the governed code"
)]
pub struct SetArgs {
    /// An id, alias, path, or unique slug.
    pub doc: String,
    /// key=value, key+=value (add to a list), key-=value (remove from a list).
    #[arg(required = true)]
    pub assignments: Vec<String>,
    /// Show the change without writing.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Args)]
#[command(
    after_help = "Examples:\n  spec mv adr:root/20260101000000 --write      # to where its governs says\n  spec mv sot:shop/cart --slug checkout-cart --write\n  spec mv sot:shop/cart --tier committed --write   # promote a local document\n  spec mv sot:shop/cart --slug basket --cited-paths"
)]
pub struct MvArgs {
    /// An id, alias, path, or unique slug.
    pub doc: String,
    /// Target owner; computed from rel.governs when absent.
    #[arg(long)]
    pub owner: Option<String>,
    /// New slug.
    #[arg(long)]
    pub slug: Option<String>,
    /// Move between the committed tree and the git-ignored local one: `committed` promotes a
    /// local document, `local` takes one out of the commit.
    #[arg(long, value_parser = ["committed", "local"])]
    pub tier: Option<String>,
    /// Also list committed Markdown files outside the documents that cite the old path
    /// (`file:line`); they are reported, not rewritten. Off by default: it reads every such file.
    #[arg(long)]
    pub cited_paths: bool,
    /// Apply the move (otherwise a preview).
    #[arg(long)]
    pub write: bool,
}

#[derive(Debug, Clone, Args)]
#[command(
    after_help = "Examples:\n  spec fix --staged --write\n  spec fix --rule L1 --write services/shop/.spec/plan/x.plan.md\n  spec fix --rule L1 --pick adr:root/20260412083000 --write services/shop/.spec/plan/x.plan.md:41"
)]
pub struct FixArgs {
    /// Limit to files or `path:line` locations.
    pub targets: Vec<String>,
    /// Only fixes for these rules (repeatable).
    #[arg(long = "rule", id = "rule")]
    pub rules: Vec<String>,
    /// Only documents changed in the index.
    #[arg(long)]
    pub staged: bool,
    /// For an ambiguous reference: the id to use.
    #[arg(long)]
    pub pick: Option<String>,
    /// Apply the fixes (otherwise a preview).
    #[arg(long)]
    pub write: bool,
}

#[derive(Debug, Clone, Args)]
#[command(
    after_help = "Examples:\n  spec attach design:shop/20260101000000 boards/cart.webp --write\n  spec attach evidence:root/20260101000000 run.jsonl --store --write"
)]
pub struct AttachArgs {
    /// An id, alias, path, or unique slug.
    pub doc: String,
    /// Files to attach.
    #[arg(required = true)]
    pub files: Vec<PathBuf>,
    /// Subdirectory inside the bundle.
    #[arg(long, default_value = "")]
    pub into: String,
    /// Keep in the store regardless of size.
    #[arg(long, conflicts_with = "repo")]
    pub store: bool,
    /// Keep in the repository (refused over the size limit).
    #[arg(long)]
    pub repo: bool,
    /// Copy, upload and register (otherwise a preview).
    #[arg(long)]
    pub write: bool,
}

#[derive(Debug, Clone, Args)]
#[command(
    after_help = "Examples:\n  spec init\n  spec init --owner 'services/*={1}' --owner 'libs/*=lib.{1}:manifest' --root ws\n  spec init --upgrade          # preview renamed keys and the schema number"
)]
pub struct InitArgs {
    /// Owner kinds as `pattern=id-template`, with `:manifest` to require a package manifest.
    #[arg(long = "owner")]
    pub owners: Vec<String>,
    /// Id of the repository root owner.
    #[arg(long, default_value = "root")]
    pub root: String,
    /// Upgrade an existing spec.toml to this version: renamed keys and the schema number,
    /// keeping comments. A preview without --write.
    #[arg(long, conflicts_with_all = ["owners"])]
    pub upgrade: bool,
    /// Write spec.toml (otherwise a preview).
    #[arg(long)]
    pub write: bool,
}

#[derive(Debug, Clone, Args)]
#[command(
    after_help = "Examples:\n  spec import --map import.toml --plan\n  spec import --map import.toml --apply --write"
)]
pub struct ImportArgs {
    /// The import map (TOML).
    #[arg(long)]
    pub map: PathBuf,
    /// Print the plan rows (the default).
    #[arg(long, conflicts_with = "apply")]
    pub plan: bool,
    /// Apply the plan (with --write).
    #[arg(long)]
    pub apply: bool,
    /// Write files (otherwise a preview of --apply).
    #[arg(long)]
    pub write: bool,
}

#[derive(Debug, Clone, Args)]
#[command(
    after_help = "Examples:\n  spec edit adr:shop/20260101000000 --section Decision --text \"Use Postgres.\"\n  spec edit adr:shop/20260101000000 --append --file addendum.md --write\n  cat body.md | spec edit research:shop/20260101000000 --write\n  spec edit adr:shop/20260101000000 --section Decision --replace --write < decision.md\n  spec edit plan:shop/20260101000000 --new-section Risks --after Goal --text \"- none yet\" --write\n  spec edit plan:shop/20260101000000 --section Tasks --row A1 --cell Status=done --write\n  spec edit plan:shop/20260101000000 --row A9 --append --text \"A9 | next | new idea\" --write\n\nWithout --text or --file, text piped or redirected into standard input is read; at a terminal one of them is required."
)]
pub struct EditArgs {
    /// An id, alias, path, or unique slug.
    pub doc: String,
    /// Change only this section (heading text or anchor); without it the whole body.
    #[arg(long)]
    pub section: Option<String>,
    /// Add a new section with this heading (refused if one exists).
    #[arg(long, conflicts_with_all = ["section", "append", "replace"])]
    pub new_section: Option<String>,
    /// With --new-section: place it after this section, at the same heading level.
    #[arg(long, requires = "new_section")]
    pub after: Option<String>,
    /// Change one table row: the row whose first cell is this text (with --section to pick the table).
    #[arg(long, conflicts_with = "new_section")]
    pub row: Option<String>,
    /// With --row: set this column (by header) to a value, `Status=done`; repeatable. Without it the
    /// text is the whole new row, `A1 | done | note`.
    #[arg(long, requires = "row", value_name = "COLUMN=VALUE")]
    pub cell: Vec<String>,
    /// Add after the existing content instead of replacing it (with --row: add the row when missing).
    #[arg(long)]
    pub append: bool,
    /// Replace the content (the default; spelled out for scripts).
    #[arg(long, conflicts_with = "append")]
    pub replace: bool,
    /// The new Markdown text.
    #[arg(long, conflicts_with = "file")]
    pub text: Option<String>,
    /// Read the new Markdown text from a file; `-` reads standard input, as does no --text or --file when input is piped.
    #[arg(long)]
    pub file: Option<PathBuf>,
    /// Apply the change; without it only the preview is shown.
    #[arg(long)]
    pub write: bool,
}

#[derive(Debug, Clone, Args)]
#[command(
    after_help = "Copies each [[assets]] root's copy_of source over that root, for example:\n  [[assets]]\n  kind = \"skill\"\n  path = \".claude/skills\"\n  copy_of = \".agents/skills\"\nOnly roots with copy_of are written, only inside that root; the source is never changed and symbolic links are refused. Files missing from the source stay unless --prune. spec fix never touches assets.\n\nExamples:\n  spec sync assets              # preview\n  spec sync assets --write\n  spec sync assets --prune --write"
)]
pub struct SyncArgs {
    /// What to sync: assets.
    #[arg(value_parser = ["assets"])]
    pub what: String,
    /// Also remove files in a copy that its source does not have.
    #[arg(long)]
    pub prune: bool,
    /// Apply the copies (otherwise a preview).
    #[arg(long)]
    pub write: bool,
}

impl NewArgs {
    /// The title from --title or the positional argument; clap requires one of them.
    pub fn title(&self) -> &str {
        self.title.as_deref().or(self.title_text.as_deref()).unwrap_or_default()
    }
}
