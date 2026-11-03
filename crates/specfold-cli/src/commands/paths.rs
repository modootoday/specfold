//! Path resolution for the paths commands are given, kept in one place for every command.

/// An absolute path for a file that may not exist yet: the nearest existing ancestor is resolved.
pub fn absolute(p: &std::path::Path) -> std::path::PathBuf {
    let p = if p.is_absolute() { p.to_path_buf() } else { std::env::current_dir().unwrap_or_default().join(p) };
    let mut rest = vec![];
    let mut cur = p.as_path();
    loop {
        if let Ok(c) = cur.canonicalize() {
            return rest.iter().rev().fold(c, |acc, part| acc.join(part));
        }
        match (cur.parent(), cur.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name.to_os_string());
                cur = parent;
            }
            _ => return p,
        }
    }
}
