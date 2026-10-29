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
    /// The exporter plugin.
    pub plugin: String,
    /// The format, when the plugin offers several.
    #[arg(long)]
    pub format: Option<String>,
    /// Directory the plugin writes into.
    #[arg(long)]
    pub out: Option<std::path::PathBuf>,
}
