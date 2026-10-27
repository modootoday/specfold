//! The alias table: `old<TAB>new-id` lines written once by an import, then frozen.

use super::types::AliasTable;
use crate::config::Config;

pub fn load(cfg: &Config) -> AliasTable {
    let Some(file) = cfg.raw.aliases.file.clone() else { return AliasTable::default() };
    let mut table = AliasTable { file: Some(file.clone()), ..Default::default() };
    let Ok(text) = std::fs::read_to_string(cfg.root.join(&file)) else { return table };
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
