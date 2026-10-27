//! Project configuration (spec.toml).

pub mod defaults;
pub mod paths;
pub mod reader;
pub mod starter;
pub mod types;

pub use reader::Config;
pub use types::{AuditSection, GuardDef, OwnerKind, PluginDef, RawConfig, StoreDef};
