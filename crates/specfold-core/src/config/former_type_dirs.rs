//! Type directories that spec.toml at HEAD declares and the working spec.toml no longer does.
//! Their documents are still read, so S3 can refuse the change until `spec mv` takes them over;
//! once the change is committed the directory is forgotten, as any other.

use super::Config;
use super::defaults::CONFIG_FILE;
use super::type_dirs::normalize;
use crate::paths::is_under;

/// `(directory, type)` for each type directory HEAD's spec.toml has and the working one dropped
/// or moved, the directory relative to an owner's root. Empty outside a repository.
pub fn former(cfg: &Config) -> Vec<(String, String)> {
    let head = crate::workspace::vcs::show(&cfg.spec_home, "HEAD", &format!("./{CONFIG_FILE}"));
    let Some(table) = head.and_then(|t| t.parse::<toml::Table>().ok()) else { return vec![] };
    let Some(types) = table.get("types").and_then(|t| t.as_table()) else { return vec![] };
    let mut taken: Vec<String> = cfg.type_dirs().into_iter().map(|(d, _)| d).collect();
    taken.extend([cfg.spec_dir(), cfg.local_dir()].iter().filter_map(|d| crate::paths::normalize(d)));
    let mut out = vec![];
    for (name, t) in types {
        let Some(dir) = t.get("dir").and_then(|d| d.as_str()).and_then(|d| normalize(d).ok()) else { continue };
        let apart = taken.iter().all(|c| !is_under(&dir, c) && !is_under(c, &dir));
        if apart && cfg.registry.get(name).is_some() {
            taken.push(dir.clone());
            out.push((dir, name.clone()));
        }
    }
    out
}
