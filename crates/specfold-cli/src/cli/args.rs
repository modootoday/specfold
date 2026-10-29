//! The command line. This definition is also the source of `spec help --json` and of the MCP
//! tool list, so the three can never disagree.

pub mod global;
pub mod read;
pub mod setup;
pub mod verify;
pub mod write;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "spec",
    version = concat!(env!("CARGO_PKG_VERSION"), " (", env!("SPECFOLD_COMMIT"), ")"),
    about = "One CLI for a repository's design, planning and maintenance documents.",
    long_about = "Typed Markdown documents stored with the code they govern, a graph derived from them, checks, guards and audits. Every command is non-interactive, prints text or --json, and ends with a suggested next command.",
    disable_help_subcommand = true
)]
pub struct Cli {
    #[command(flatten)]
    pub global: global::Global,
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// The documents that govern code paths: read this before changing code.
    Context(read::ContextArgs),
    /// Find documents by words, type, owner, status or tag.
    Find(read::FindArgs),
    /// Frontmatter, outline, one section, or relations of a document.
    Show(read::ShowArgs),
    /// Resolve an id, alias, path or slug to one document.
    #[command(
        after_help = "Examples:\n  spec resolve sot:services/shop/cart-totals\n  spec resolve services/shop/.spec/sot/cart-totals.sot.md"
    )]
    Resolve(read::RefArgs),
    /// Graph queries: impact, chain, governs; or the whole graph.
    Graph(read::GraphArgs),
    /// Owners, their documents and where public owners store theirs.
    Owners(read::OwnersArgs),
    /// Agent skills, rules, subagents and instruction files declared under [[assets]].
    Assets(read::AssetsArgs),
    /// The history of a document's status and relations, from git.
    Log(read::LogArgs),
    /// Create a document at the place its owner and type decide.
    New(write::NewArgs),
    /// Change frontmatter with the lifecycle enforced.
    Set(write::SetArgs),
    /// Replace or append to a section or the whole body; previews until --write.
    Edit(write::EditArgs),
    /// Move or rename a document and rewrite every reference to it.
    Mv(write::MvArgs),
    /// Apply fixes that need no judgement.
    Fix(write::FixArgs),
    /// Copy asset sources over the [[assets]] roots that declare copy_of; previews until --write.
    Sync(write::SyncArgs),
    /// Register files with a document, in the repository or the store.
    Attach(write::AttachArgs),
    /// Download a document's stored attachments, verified by hash.
    #[command(after_help = "Examples:\n  spec fetch evidence:services/shop/20261030021100")]
    Fetch(read::RefArgs),
    /// A document's attachments and where each is kept.
    #[command(after_help = "Examples:\n  spec files evidence:services/shop/20261030021100")]
    Files(read::RefArgs),
    /// Write a starter spec.toml, or upgrade an existing one.
    Init(write::InitArgs),
    /// Bring documents from another layout in through an import map.
    Import(write::ImportArgs),
    /// Run the rules.
    Check(verify::CheckArgs),
    /// Run sot verify commands.
    Verify(verify::VerifyArgs),
    /// Run guards over files and commit messages.
    Guard(verify::GuardArgs),
    /// Diagnose what passes the rules yet needs attention; build review packs; record audits.
    Audit(verify::AuditArgs),
    /// Counts of documents, references and attachments.
    #[command(after_help = "Examples:\n  spec census\n  spec census --legacy")]
    Census(verify::CensusArgs),
    /// Read-only setup diagnosis: configuration, conflict markers, ignored local documents.
    #[command(
        long_about = "Read-only setup diagnosis: configuration, conflict markers, ignored local documents, \
                      ids defined in both .spec and .spec.local, and references left by a half-done mv.\n\n\
                      A reference counts as moved only when a document of the same type and key exists under \
                      another owner; a changed slug is not detected (spec check reports it as a broken link).",
        after_help = "Examples:\n  spec doctor\n  spec doctor --json"
    )]
    Doctor,
    /// Why a rule exists and how to fix it; or the configuration keys.
    Explain(setup::ExplainArgs),
    /// Document types and their frontmatter; or output schema names.
    #[command(after_help = "Examples:\n  spec schema adr\n  spec schema output")]
    Schema(setup::SchemaArgs),
    /// Commands and arguments; with --json, the whole command tree.
    #[command(after_help = "Examples:\n  spec help edit\n  spec help --json\n  spec help --markdown")]
    Help(setup::HelpArgs),
    /// The lines to put in an agent's instruction file.
    Agents(setup::AgentsArgs),
    /// Hook snippets for git, pre-commit, lefthook or husky.
    Hook(setup::HookArgs),
    /// Configured plugins and what they provide.
    #[command(after_help = "Examples:\n  spec plugins\n  spec plugins describe <name>")]
    Plugins(setup::PluginsArgs),
    /// Export through an exporter plugin.
    #[command(after_help = "Examples:\n  spec export <plugin> --out site")]
    Export(setup::ExportArgs),
    /// Serve these commands as MCP tools over stdio.
    #[command(after_help = "Examples:\n  spec mcp")]
    Mcp,
    /// Serve check findings as editor diagnostics (Language Server Protocol over stdio).
    #[command(
        long_about = "Serve check findings as editor diagnostics (Language Server Protocol over stdio).\n\nStarted by an editor's LSP client. Open, changed and saved documents are checked with their unsaved text; each publishes the findings `spec check <FILE>` would list for that file. Findings with a replacement fix offer it as a quick fix on the unsaved text.",
        after_help = "Examples:\n  spec lsp\n  spec lsp --dir /path/to/repo"
    )]
    Lsp,
}
