//! Arguments of the commands that describe specfold and connect it to other tools.

use clap::Args;

#[derive(Debug, Clone, Args)]
#[command(after_help = "Examples:\n  spec explain L1\n  spec explain config\n  spec explain        # every rule")]
pub struct ExplainArgs {
    /// A rule id (S1, L1 …), a plugin rule (x.name.id), or `config`.
    pub topic: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct SchemaArgs {
    /// A document type, or `output` for every --json schema name.
    #[arg(id = "type")]
    pub ty: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct HelpArgs {
    /// One command.
    pub command: Option<String>,
    /// Print the command table as Markdown, as embedded in the README.
    #[arg(long)]
    pub markdown: bool,
    /// Print llms.txt: the description and every command, linked to the reference.
    #[arg(long, conflicts_with = "markdown")]
    pub llms: bool,
    /// List every command, not only the core ones (--json, --markdown and --llms always do).
    #[arg(long)]
    pub all: bool,
}

#[derive(Debug, Clone, Args)]
#[command(after_help = "Examples:\n  spec agents\n  spec agents --for cursor")]
pub struct AgentsArgs {
    /// claude, codex, gemini, cursor, copilot or generic.
    #[arg(long = "for", default_value = "generic")]
    pub target: String,
}

#[derive(Debug, Clone, Args)]
#[command(after_help = "Examples:\n  spec hook print --for git\n  spec hook print --for pre-commit")]
pub struct HookArgs {
    /// Only `print` exists: specfold never installs hooks itself.
    pub action: String,
    /// git, pre-commit, lefthook or husky.
    #[arg(long = "for", default_value = "git")]
    pub target: String,
}

#[derive(Debug, Clone, Args)]
pub struct PluginsArgs {
    /// describe or test, followed by a plugin name.
    pub action: Option<String>,
    /// The plugin.
    pub name: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct ExportArgs {
    /// `markdown` for the built-in exporter, else an exporter plugin.
    pub plugin: String,
    /// The format, when the plugin offers several.
    #[arg(long)]
    pub format: Option<String>,
    /// Directory to write into: empty, or an earlier export (it has .specfold-export.json).
    #[arg(long)]
    pub out: Option<std::path::PathBuf>,
    /// markdown: obsidian, mkdocs, docusaurus or github-wiki.
    #[arg(long)]
    pub target: Option<String>,
    /// Include documents in the local (git-ignored) directory; left out by default, for plugins too.
    #[arg(long)]
    pub local: bool,
    /// markdown: start each page with a note naming its source file.
    #[arg(long)]
    pub banner: bool,
    /// markdown: `unresolved` fails, writing nothing, when a reference leads nowhere in the tree.
    #[arg(long)]
    pub fail_on: Option<String>,
    /// Skip the built-in secrets check and the guards marked `public = true`, which otherwise
    /// read everything before it leaves.
    #[arg(long)]
    pub no_guard: bool,
    /// Confirm an action that deletes, overwrites, sends files out, or writes more than [write] max_files files (with its write flag).
    #[arg(long)]
    pub yes: bool,
}
