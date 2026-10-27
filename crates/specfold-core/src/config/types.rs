//! The shape of spec.toml. Everything project-specific lives here; the code only knows the
//! kinds of rules.

use super::defaults;
use crate::model::registry::TypeOverride;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpecSection {
    #[serde(default = "defaults::schema")]
    pub schema: u32,
    /// Directory name that holds an owner's documents.
    #[serde(default = "defaults::spec_dir")]
    pub dir: String,
}

impl Default for SpecSection {
    fn default() -> Self {
        SpecSection { schema: defaults::schema(), dir: defaults::spec_dir() }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerKind {
    /// A pattern of directories relative to the repository root, e.g. `services/*` or `libs/*/*`.
    pub pattern: String,
    /// The owner id template; `{1}`, `{2}` … are the `*` segments in order.
    pub id: String,
    /// Only directories that contain one of the manifests count.
    #[serde(default)]
    pub manifest: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnersSection {
    /// Id of the repository root as an owner.
    #[serde(default = "defaults::root_owner")]
    pub root: String,
    #[serde(default)]
    pub kinds: Vec<OwnerKind>,
    /// Replaces the default manifest list.
    pub manifests: Option<Vec<String>>,
    /// Directories whose code is gone; documents that only govern them belong to the root.
    #[serde(default)]
    pub retired: Vec<String>,
}

impl Default for OwnersSection {
    fn default() -> Self {
        OwnersSection { root: defaults::root_owner(), kinds: vec![], manifests: None, retired: vec![] }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PublicSection {
    /// Paths published to the world. No spec document may live under them.
    #[serde(default)]
    pub paths: Vec<String>,
    /// Where documents about a public path go instead.
    #[serde(default)]
    pub companion: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LocalSection {
    #[serde(default = "defaults::local_dir")]
    pub path: String,
}

impl Default for LocalSection {
    fn default() -> Self {
        LocalSection { path: defaults::local_dir() }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalRoot {
    /// A directory outside the repository, `~` expanded.
    pub path: String,
    /// Only `cite-check` exists: report citations that no longer resolve.
    #[serde(default = "defaults::cite_check")]
    pub mode: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FilesSection {
    /// Larger attachments go to the store.
    #[serde(default = "defaults::inline_max_bytes")]
    pub inline_max_bytes: u64,
    pub store: Option<StoreDef>,
}

impl Default for FilesSection {
    fn default() -> Self {
        FilesSection { inline_max_bytes: defaults::inline_max_bytes(), store: None }
    }
}

/// Where attachments too large for the repository live. specfold speaks no network protocol
/// itself: a `dir` store is a plain directory, a `command` store runs the tool you already trust
/// for your bucket (rclone, aws, gsutil, wrangler …).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoreDef {
    /// `dir` or `command`.
    pub kind: String,
    /// For `dir`: the directory, `~` expanded.
    pub path: Option<String>,
    /// For `command`: argv templates. `{src}`/`{dst}` are local paths, `{key}` is the object key.
    pub put: Option<Vec<String>>,
    pub get: Option<Vec<String>>,
    pub exists: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ForbidSection {
    /// Globs that must not exist anywhere in the repository.
    #[serde(default)]
    pub paths: Vec<String>,
    /// Regexes that must not appear inside spec documents (old id schemes, for example).
    #[serde(default)]
    pub patterns: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AliasSection {
    /// A frozen `old<TAB>new-id` table written by `spec import`.
    pub file: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GuardDef {
    pub name: String,
    #[serde(default)]
    pub about: String,
    /// Patterns the guard looks at; empty means the whole repository.
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    /// Default range when none is given: `staged`, `push` or `all`.
    #[serde(default = "defaults::guard_range")]
    pub range: String,
    /// Case-insensitive terms, inline.
    #[serde(default)]
    pub deny_terms: Vec<String>,
    /// A file of terms, one per line, `#` comments; may live outside the repository.
    pub deny_terms_file: Option<String>,
    /// An environment variable holding terms separated by newlines or commas.
    pub deny_terms_env: Option<String>,
    #[serde(default)]
    pub deny_regex: Vec<String>,
    #[serde(default)]
    pub deny_files: Vec<String>,
    pub max_file_bytes: Option<u64>,
    /// Built-in secret patterns (private keys, cloud and service tokens).
    #[serde(default)]
    pub secrets: bool,
    /// Also scan commit messages and author names in the range.
    #[serde(default)]
    pub commit_messages: bool,
    #[serde(default = "defaults::guard_level")]
    pub level: String,
}

/// An external program that extends specfold over the `specfold.plugin.v1` JSON protocol.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PluginDef {
    pub name: String,
    /// argv; relative programs are resolved against the repository root.
    pub cmd: Vec<String>,
    #[serde(default = "defaults::plugin_timeout_ms")]
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AuditSection {
    /// Governed code changed this many days after the document's `updated`.
    #[serde(default = "defaults::stale_days")]
    pub stale_days: i64,
    /// An active plan with no update for this many days.
    #[serde(default = "defaults::stalled_days")]
    pub stalled_days: i64,
    /// A draft older than this.
    #[serde(default = "defaults::draft_days")]
    pub draft_days: i64,
    /// Text that marks unfinished work inside documents.
    #[serde(default = "defaults::open_item_patterns")]
    pub open_item_patterns: Vec<String>,
}

impl Default for AuditSection {
    fn default() -> Self {
        AuditSection {
            stale_days: defaults::stale_days(),
            stalled_days: defaults::stalled_days(),
            draft_days: defaults::draft_days(),
            open_item_patterns: defaults::open_item_patterns(),
        }
    }
}

/// Files outside spec documents whose citations of documents are checked (rule L6): code
/// comments, READMEs, agent instruction files.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CitesSection {
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    /// Regexes for lines that record history ("moved to …"); citations on them are not judged.
    #[serde(default = "defaults::history_patterns")]
    pub history: Vec<String>,
}

impl Default for CitesSection {
    fn default() -> Self {
        CitesSection { paths: vec![], exclude: vec![], history: defaults::history_patterns() }
    }
}

/// How `spec check` judges an existing pile.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CheckSection {
    /// A committed file of known findings (`spec check --write-baseline`); only new ones fail.
    pub baseline: Option<String>,
    /// Paths where every finding counts, baseline or not.
    #[serde(default)]
    pub strict: Vec<String>,
    /// Rule id → "error", "warn" or "off".
    #[serde(default)]
    pub severity: BTreeMap<String, String>,
}

/// How `spec verify` runs the commands documents name.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerifySection {
    /// Default deadline per command; a document's `verify_timeout_ms` overrides it.
    #[serde(default = "defaults::verify_timeout_ms")]
    pub timeout_ms: u64,
    /// Commands run at once; 0 means the number of CPUs, at most 8.
    #[serde(default)]
    pub jobs: usize,
    /// Regexes for heavy commands (test suites, type checks) that run one at a time in their own lane.
    #[serde(default)]
    pub serial: Vec<String>,
    /// Paths whose change runs every verify command (lockfiles, shared manifests, the checks themselves).
    #[serde(default)]
    pub full_run_on: Vec<String>,
}

impl Default for VerifySection {
    fn default() -> Self {
        VerifySection { timeout_ms: defaults::verify_timeout_ms(), jobs: 0, serial: vec![], full_run_on: vec![] }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RawConfig {
    #[serde(default)]
    pub spec: SpecSection,
    #[serde(default)]
    pub owners: OwnersSection,
    #[serde(default)]
    pub public: PublicSection,
    #[serde(default)]
    pub local: LocalSection,
    #[serde(default)]
    pub external: BTreeMap<String, ExternalRoot>,
    #[serde(default)]
    pub files: FilesSection,
    #[serde(default)]
    pub types: BTreeMap<String, TypeOverride>,
    #[serde(default)]
    pub forbid: ForbidSection,
    #[serde(default)]
    pub aliases: AliasSection,
    #[serde(default)]
    pub guards: Vec<GuardDef>,
    #[serde(default)]
    pub plugins: Vec<PluginDef>,
    #[serde(default)]
    pub audit: AuditSection,
    #[serde(default)]
    pub verify: VerifySection,
    #[serde(default)]
    pub check: CheckSection,
    #[serde(default)]
    pub cites: CitesSection,
}
