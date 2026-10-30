//! Markdown pipe tables: find a row by the text of its first cell and change it, so an agent can
//! update one line of a status table without rewriting the section.

use anyhow::{Result, bail};

/// The cells of a table line, or None when the line is not a table row.
pub fn cells(line: &str) -> Option<Vec<String>> {
    let t = line.trim();
    if !t.starts_with('|') {
        return None;
    }
    let mut out = vec![];
    let mut cur = String::new();
    let mut chars = t[1..].chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                cur.push_str("\\|");
                chars.next();
            }
            '|' => out.push(std::mem::take(&mut cur).trim().to_string()),
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    Some(out)
}

fn is_separator(cells: &[String]) -> bool {
    !cells.is_empty()
        && cells.iter().all(|c| {
            let c = c.trim_matches(':');
            !c.is_empty() && c.chars().all(|ch| ch == '-')
        })
}

fn render(cells: &[String]) -> String {
    format!("| {} |", cells.join(" | "))
}

fn norm(s: &str) -> String {
    s.trim().trim_matches('`').trim_matches('*').trim().to_lowercase()
}

#[derive(Debug)]
struct Table {
    header: Vec<String>,
    /// Line indexes of the data rows.
    rows: Vec<usize>,
    /// Line index after the table.
    end: usize,
}

fn tables(lines: &[&str], lo: usize, hi: usize) -> Vec<Table> {
    let mut out = vec![];
    let mut i = lo;
    let mut fence = crate::markdown::Fence::default();
    while i < hi {
        let l = lines[i].trim_start();
        if fence.step(l) {
            i += 1;
            continue;
        }
        if !fence.inside()
            && let Some(header) = cells(lines[i])
            && i + 1 < hi
            && cells(lines[i + 1]).is_some_and(|s| is_separator(&s))
        {
            let mut j = i + 2;
            let mut rows = vec![];
            while j < hi && cells(lines[j]).is_some() {
                rows.push(j);
                j += 1;
            }
            out.push(Table { header, rows, end: j });
            i = j;
            continue;
        }
        i += 1;
    }
    out
}

/// How to change the row.
#[derive(Debug, Clone, Default)]
pub struct RowChange {
    /// The first-cell text that names the row.
    pub key: String,
    /// Column header = new value; empty means `row` holds the whole new row.
    pub set: Vec<(String, String)>,
    pub row: Option<String>,
    /// Add the row at the end of the table when no row has the key.
    pub add: bool,
}

/// The edited lines plus the row before and after (empty before when added).
pub fn edit_row(lines: &[&str], lo: usize, hi: usize, ch: &RowChange) -> Result<(Vec<String>, String, String)> {
    let found = tables(lines, lo, hi);
    if found.is_empty() {
        bail!("no table here; --row edits a Markdown pipe table");
    }
    let key = norm(&ch.key);
    let hits: Vec<(&Table, usize)> = found
        .iter()
        .flat_map(|t| t.rows.iter().map(move |&r| (t, r)))
        .filter(|(_, r)| cells(lines[*r]).and_then(|c| c.first().map(|f| norm(f) == key)).unwrap_or(false))
        .collect();
    let build = |t: &Table, old: Option<Vec<String>>| -> Result<Vec<String>> {
        if let Some(text) = &ch.row {
            let t2 = text.trim();
            let line = if t2.starts_with('|') { t2.to_string() } else { format!("| {t2} |") };
            let new = cells(&line).unwrap_or_default();
            if new.len() != t.header.len() {
                bail!(
                    "the row has {} cells; the table has {} columns ({})",
                    new.len(),
                    t.header.len(),
                    t.header.join(" | ")
                );
            }
            return Ok(new);
        }
        let mut row = old.unwrap_or_else(|| {
            let mut r = vec![String::new(); t.header.len()];
            r[0] = ch.key.trim().to_string();
            r
        });
        row.resize(t.header.len().max(row.len()), String::new());
        for (col, val) in &ch.set {
            let Some(i) = t.header.iter().position(|h| norm(h) == norm(col)) else {
                bail!("no column '{col}' (columns: {})", t.header.join(" | "));
            };
            row[i] = val.replace('|', "\\|");
        }
        Ok(row)
    };
    let mut out: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    match hits.as_slice() {
        [(t, r)] => {
            let new = render(&build(t, cells(lines[*r]))?);
            let before = lines[*r].to_string();
            out[*r] = new.clone();
            Ok((out, before, new))
        }
        [] if ch.add => {
            if found.len() > 1 {
                bail!("{} tables here; name the section with --section", found.len());
            }
            let t = &found[0];
            let new = render(&build(t, None)?);
            out.insert(t.end, new.clone());
            Ok((out, String::new(), new))
        }
        [] => {
            let keys: Vec<String> =
                found.iter().flat_map(|t| &t.rows).filter_map(|&r| cells(lines[r])?.first().cloned()).collect();
            bail!("no row '{}' (rows: {}); --append adds it", ch.key, keys.join(" | "))
        }
        many => bail!("'{}' names {} rows; name the section with --section", ch.key, many.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: &str =
        "intro\n\n| Id | Status | Note |\n|---|:-:|---|\n| `A1` | next | a \\| b |\n| A2 | waiting | |\n\nafter\n";

    fn run(ch: RowChange) -> Result<String> {
        let lines: Vec<&str> = T.lines().collect();
        edit_row(&lines, 0, lines.len(), &ch).map(|(l, _, _)| l.join("\n"))
    }

    #[test]
    fn sets_a_cell_by_key_and_column() {
        let out =
            run(RowChange { key: "a1".into(), set: vec![("status".into(), "done".into())], ..Default::default() })
                .unwrap();
        assert!(out.contains("| `A1` | done | a \\| b |"), "{out}");
    }

    #[test]
    fn replaces_or_adds_whole_rows() {
        let out =
            run(RowChange { key: "A2".into(), row: Some("A2 | done | ok".into()), ..Default::default() }).unwrap();
        assert!(out.contains("| A2 | done | ok |\n\nafter"), "{out}");
        let out = run(RowChange {
            key: "A3".into(),
            set: vec![("Status".into(), "next".into())],
            add: true,
            ..Default::default()
        })
        .unwrap();
        assert!(out.contains("| A2 | waiting | |\n| A3 | next |  |\n\nafter"), "{out}");
        assert!(run(RowChange { key: "A3".into(), row: Some("A3 | x".into()), ..Default::default() }).is_err());
        assert!(
            run(RowChange { key: "A1".into(), set: vec![("Owner".into(), "x".into())], ..Default::default() }).is_err()
        );
    }
}
