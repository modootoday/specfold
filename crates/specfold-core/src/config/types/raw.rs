//! spec.toml as written, one field per section.

use super::{
    AliasSection, AssetRoot, AuditSection, CheckSection, CitesSection, ExternalRoot, FilesSection, ForbidSection,
    GuardDef, LocalSection, OwnersSection, PluginDef, PublicSection, RemoteRoot, SpecSection, TranslationsSection,
    VerifySection,
};
use crate::model::registry::TypeOverride;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
    /// Other repositories whose documents are named as `<name>:<id>`.
    #[serde(default)]
    pub remotes: BTreeMap<String, RemoteRoot>,
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
    #[serde(default)]
    pub translations: TranslationsSection,
    /// Agent assets read beside the documents.
    #[serde(default)]
    pub assets: Vec<AssetRoot>,
}
