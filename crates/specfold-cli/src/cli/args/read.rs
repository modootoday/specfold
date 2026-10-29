//! Arguments of the commands that only read.

use clap::Args;

#[derive(Debug, Clone, Args)]
#[command(
    after_help = "Examples:\n  spec context services/shop/src/cart/total.rs\n  spec context services/shop --json"
)]
pub struct ContextArgs {
    /// Code paths you are about to change.
    #[arg(required = true)]
    pub paths: Vec<String>,
    /// Include superseded and archived documents.
    #[arg(long)]
    pub all: bool,
}

#[derive(Debug, Clone, Args)]
#[command(
    after_help = "Examples:\n  spec find checkout\n  spec find --type adr --owner shop\n  spec find retry budget --text"
)]
pub struct FindArgs {
    /// Words that must all match the id, title, slug, aliases or tags.
    pub words: Vec<String>,
    /// Only these document types (repeatable).
    #[arg(long = "type", id = "type", value_name = "TYPE")]
    pub types: Vec<String>,
    /// Only documents of these owners (repeatable).
    #[arg(long = "owner", id = "owner", value_name = "OWNER")]
    pub owners: Vec<String>,
    /// Only these statuses (repeatable).
    #[arg(long = "status", id = "status", value_name = "STATUS")]
    pub statuses: Vec<String>,
    /// Only documents with these tags (repeatable).
    #[arg(long = "tag", id = "tag", value_name = "TAG")]
    pub tags: Vec<String>,
    /// Include superseded, archived and rejected documents.
    #[arg(long)]
    pub all: bool,
    /// Also search body text.
    #[arg(long)]
    pub text: bool,
}

#[derive(Debug, Clone, Args)]
#[command(
    after_help = "Examples:\n  spec show sot:shop/cart\n  spec show sot:shop/cart --section rules\n  spec show adr:shop/20260101000000 --rel\n  spec show plan:shop/20260101000000 --tasks"
)]
pub struct ShowArgs {
    /// An id, alias, path, or unique slug.
    pub doc: String,
    /// Only the section with this heading or anchor.
    #[arg(long)]
    pub section: Option<String>,
    /// Show incoming and outgoing relations.
    #[arg(long)]
    pub rel: bool,
    /// The whole body (otherwise frontmatter and outline).
    #[arg(long)]
    pub body: bool,
    /// List the task items (`- [ ]`, `- [x]`) with their lines and whether each is done.
    #[arg(long)]
    pub tasks: bool,
}

#[derive(Debug, Clone, Args)]
pub struct RefArgs {
    /// An id, alias, path, or unique slug.
    pub doc: String,
}

#[derive(Debug, Clone, Args)]
#[command(
    after_help = "Without a query the whole graph prints as a tree from the documents nothing points at: each line is a relation and its target, governs lines name code paths, and a document already shown ends in (*).\nimpact lists what reaches a document, with the relation and the distance; chain shows the supersede chain both ways.\n\nExamples:\n  spec graph\n  spec graph --depth 2 --ascii\n  spec graph --format box\n  COLUMNS=200 spec graph --format box --depth 3\n  spec graph --format mermaid > docs-graph.mmd\n  spec graph --format dot | dot -Tsvg > docs-graph.svg\n  spec graph impact sot:shop/cart --depth 1\n  spec graph chain adr:shop/20250101000000\n  spec graph governs services/shop/src\n  spec graph --json > docs-graph.out"
)]
pub struct GraphArgs {
    /// impact, chain or governs. Without one, prints the whole graph.
    pub query: Option<String>,
    /// The document (impact, chain) or path (governs).
    pub target: Option<String>,
    /// The whole graph as a tree (default), boxes in layers (fits COLUMNS), Mermaid or Graphviz DOT text.
    #[arg(long, value_parser = ["tree", "box", "mermaid", "dot"], default_value = "tree")]
    pub format: String,
    /// Stop this many relations away (tree and impact).
    #[arg(long)]
    pub depth: Option<usize>,
    /// ASCII lines instead of box drawing; automatic when the locale is not UTF-8.
    #[arg(long)]
    pub ascii: bool,
}

#[derive(Debug, Clone, Args)]
#[command(after_help = "Examples:\n  spec owners\n  spec owners --empty --json")]
pub struct OwnersArgs {
    /// Include owners without documents.
    #[arg(long)]
    pub empty: bool,
}

#[derive(Debug, Clone, Args)]
#[command(
    after_help = "Assets come from spec.toml, for example:\n  [[assets]]\n  kind = \"skill\"\n  path = \".agents/skills\"\nKinds: skill (SKILL.md in a directory), rule (.md or .mdc), agent (.md), instructions (AGENTS.md, CLAUDE.md).\nEach asset lists the documents it names by id or path (docs); spec show <id> lists the assets that name it. Assets are only read here; spec sync assets writes roots that declare copy_of.\n\nExamples:\n  spec assets\n  spec assets --kind skill --json"
)]
pub struct AssetsArgs {
    /// Only this kind: skill, rule, agent or instructions.
    #[arg(long, value_parser = ["skill", "rule", "agent", "instructions"])]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Args)]
#[command(after_help = "Examples:\n  spec log sot:shop/cart\n  spec log --since 2026-01-01 --type adr")]
pub struct LogArgs {
    /// One document; without it, every document changed since --since.
    pub doc: Option<String>,
    /// Only changes on or after this date (YYYY-MM-DD).
    #[arg(long)]
    pub since: Option<String>,
    /// Only these document types (repeatable).
    #[arg(long = "type", id = "type")]
    pub types: Vec<String>,
}
