//! The shape of spec.toml, one module per section. Everything project-specific lives here; the
//! code only knows the kinds of rules.

mod audit;
mod files;
mod guards;
mod layout;
mod owners;
mod plugins;
mod public;
mod raw;
mod rules;
mod verify;

pub use audit::AuditSection;
pub use files::{FilesSection, StoreDef};
pub use guards::{GuardDef, GuardFixtures, RegexRule, RegexSpec};
pub use layout::{ExternalRoot, LocalSection, SpecSection};
pub use owners::{OwnerKind, OwnersSection};
pub use plugins::PluginDef;
pub use public::PublicSection;
pub use raw::RawConfig;
pub use rules::{AliasSection, CheckSection, CitesSection, ForbidSection};
pub use verify::VerifySection;
