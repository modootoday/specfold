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
    /// One-line title; also the source of the slug.
    #[arg(long)]
    pub title: String,
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
    after_help = "Examples:\n  spec set adr:shop/20260101000000 status=active\n  spec set plan:shop/20260101000000 phase=P3 rel.implements+=sot:shop/cart\n  spec set sot:shop/cart tags-=draft"
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
    after_help = "Examples:\n  spec mv adr:root/20260101000000 --write      # to where its governs says\n  spec mv sot:shop/cart --slug checkout-cart --write"
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
    after_help = "Examples:\n  spec init\n  spec init --owner 'services/*={1}' --owner 'libs/*=lib.{1}:manifest' --root ws"
)]
pub struct InitArgs {
    /// Owner kinds as `pattern=id-template`, with `:manifest` to require a package manifest.
    #[arg(long = "owner")]
    pub owners: Vec<String>,
    /// Id of the repository root owner.
    #[arg(long, default_value = "root")]
    pub root: String,
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
    after_help = "Examples:\n  spec edit adr:shop/20260101000000 --section Decision --text \"Use Postgres.\"\n  spec edit adr:shop/20260101000000 --append --file addendum.md --write\n  cat body.md | spec edit research:shop/20260101000000 --write\n  spec edit adr:shop/20260101000000 --section Decision --replace --write < decision.md\n  spec edit plan:shop/20260101000000 --new-section Risks --after Goal --text \"- none yet\" --write\n\nWithout --text or --file, text piped or redirected into standard input is read; at a terminal one of them is required."
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
    /// Add after the existing content instead of replacing it.
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
