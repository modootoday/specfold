//! Project configuration (spec.toml).

pub mod cache_gc;
pub mod clone_path;
pub mod defaults;
pub mod former_type_dirs;
pub mod paths;
pub mod private_root;
pub mod reader;
pub mod starter;
pub mod type_dirs;
pub mod type_ignore;
pub mod types;
pub mod upgrade;
pub mod validate;

pub use reader::Config;
pub use types::{AuditSection, GuardDef, OwnerKind, PluginDef, RawConfig, RegexRule, RegexSpec, StoreDef};
