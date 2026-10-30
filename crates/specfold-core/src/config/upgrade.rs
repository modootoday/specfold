//! Upgrading an older spec.toml in place: renamed keys and the schema number.
//!
//! Each release that renames or retires a key adds a row to [`RENAMED`]; the reader names the
//! new key when it meets an old one, and `spec init --upgrade` rewrites the file. Edits are
//! line-based so comments and layout survive.

use super::defaults;

/// A key moved within spec.toml: `table` is the `[table]` header it sits under ("" for top level).
pub struct Rename {
    pub table: &'static str,
    pub from: &'static str,
    pub to: &'static str,
    /// The version that made the change, shown to the user.
    pub since: &'static str,
}

/// No key has been renamed since schema 1.
pub const RENAMED: &[Rename] = &[];

/// The rename for an unknown `key` under `table`, if a newer version renamed it.
pub fn renamed<'a>(table: &str, key: &str, rows: &'a [Rename]) -> Option<&'a Rename> {
    rows.iter().find(|r| r.table == table && r.from == key)
}

/// The table a `[table]` or `[[table]]` header line names; C169: a trailing `# comment` is
/// not part of it, as TOML reads it.
fn header(t: &str) -> Option<String> {
    let rest = t.strip_prefix('[')?;
    let rest = rest.strip_prefix('[').unwrap_or(rest);
    let (name, _) = rest.split_once(']')?;
    Some(name.trim().to_string())
}

/// The upgraded text and one line per change; no changes means the file is current.
pub fn plan(text: &str, rows: &[Rename]) -> (String, Vec<String>) {
    let mut out = String::with_capacity(text.len());
    let mut changes = vec![];
    let mut table = String::new();
    for (i, line) in text.split_inclusive('\n').enumerate() {
        let t = line.trim();
        if let Some(name) = header(t) {
            table = name;
        }
        let key = t.split_once('=').map(|(k, _)| k.trim()).unwrap_or("");
        if table == "spec" && key == "schema" {
            let current = format!("schema = {}", defaults::schema());
            let old: Option<u32> = t.split_once('=').and_then(|(_, v)| v.split('#').next()?.trim().parse().ok());
            // Never lower the schema of a file written by a newer version.
            if t != current && old.is_some_and(|n| n < defaults::schema()) {
                changes.push(format!("line {}: {t} -> {current}", i + 1));
                out.push_str(&line.replacen(t, &current, 1));
                continue;
            }
        }
        if !key.is_empty()
            && let Some(r) = renamed(&table, key, rows)
        {
            changes.push(format!("line {}: [{}] {} -> {} (renamed in {})", i + 1, r.table, r.from, r.to, r.since));
            out.push_str(&line.replacen(r.from, r.to, 1));
            continue;
        }
        out.push_str(line);
    }
    (out, changes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROWS: &[Rename] = &[Rename { table: "aliases", from: "path", to: "file", since: "0.9" }];

    #[test]
    fn renames_keys_under_their_table_and_keeps_comments() {
        let text = "[spec]\nschema = 1\n# keep me\n[aliases]\npath = \"a.tsv\"  # note\n[other]\npath = 1\n";
        let (out, changes) = plan(text, ROWS);
        assert_eq!(out, "[spec]\nschema = 1\n# keep me\n[aliases]\nfile = \"a.tsv\"  # note\n[other]\npath = 1\n");
        assert_eq!(changes.len(), 1, "{changes:?}");
    }

    #[test]
    fn raises_an_old_schema_but_never_lowers_a_newer_one() {
        assert_eq!(plan("[spec]\nschema = 0\n", RENAMED).0, "[spec]\nschema = 1\n");
        assert!(plan("[spec]\nschema = 7\n", RENAMED).1.is_empty());
        // C169: a commented header is the same table, as for the reader.
        assert_eq!(plan("[spec] # main\nschema = 0\n", RENAMED).0, "[spec] # main\nschema = 1\n");
        assert_eq!(plan("[[guards]] # g\n[spec]  # s\nschema = 0 # old\n", RENAMED).1.len(), 1);
    }

    #[test]
    fn a_current_file_has_no_changes() {
        let text = "[spec]\nschema = 1\n";
        assert_eq!(plan(text, RENAMED), (text.to_string(), vec![]));
    }
}
