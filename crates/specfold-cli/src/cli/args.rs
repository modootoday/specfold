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
    version,
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
    Resolve(read::RefArgs),
    /// Graph queries: impact, chain, governs; or the whole graph.
    Graph(read::GraphArgs),
    /// Owners, their documents and where public owners store theirs.
    Owners(read::OwnersArgs),
    /// The history of a document's status and relations, from git.
    Log(read::LogArgs),
    /// Create a document at the place its owner and type decide.
    New(write::NewArgs),
    /// Change frontmatter with the lifecycle enforced.
    Set(write::SetArgs),
    /// Move or rename a document and rewrite every reference to it.
    Mv(write::MvArgs),
    /// Apply fixes that need no judgement.
    Fix(write::FixArgs),
    /// Register files with a document, in the repository or the store.
    Attach(write::AttachArgs),
    /// Download a document's stored attachments, verified by hash.
    Fetch(read::RefArgs),
    /// A document's attachments and where each is kept.
    Files(read::RefArgs),
    /// Write a starter spec.toml.
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
    Census(verify::CensusArgs),
    /// Why a rule exists and how to fix it; or the configuration keys.
    Explain(setup::ExplainArgs),
    /// Document types and their frontmatter; or output schema names.
    Schema(setup::SchemaArgs),
    /// Commands and arguments; with --json, the whole command tree.
    Help(setup::HelpArgs),
    /// The lines to put in an agent's instruction file.
    Agents(setup::AgentsArgs),
    /// Hook snippets for git, pre-commit, lefthook or husky.
    Hook(setup::HookArgs),
    /// Configured plugins and what they provide.
    Plugins(setup::PluginsArgs),
    /// Export through an exporter plugin.
    Export(setup::ExportArgs),
    /// Serve these commands as MCP tools over stdio.
    Mcp,
}
