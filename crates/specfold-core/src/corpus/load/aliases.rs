//! The alias table: `old<TAB>new-id` lines written once by an import, then frozen.

use crate::config::Config;
use crate::corpus::types::AliasTable;

pub fn load(cfg: &Config) -> AliasTable {
    let Some(file) = cfg.raw.aliases.file.clone() else { return AliasTable::default() };
    let mut table = AliasTable { file: Some(file.clone()), ..Default::default() };
    let Ok(text) = std::fs::read_to_string(cfg.spec_home.join(&file)) else { return table };
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        if let Some((old, new)) = line.split_once('\t') {
            table.map.entry(old.trim().to_string()).or_default().push(new.trim().to_string());
        }
    }
    table
}
