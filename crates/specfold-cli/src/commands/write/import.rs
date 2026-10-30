//! `spec import`: plan rows for review, or apply them.

use crate::cli::args::{global::Global, write::ImportArgs};
use crate::output::{Code, Output};
use crate::session;
use anyhow::{Context, Result, bail};
use serde_json::to_value;
use specfold_core::files::store;
use specfold_core::import::{self, ImportMap};

pub fn run(g: &Global, a: &ImportArgs) -> Result<Output> {
    if let Some(name) = &a.preset {
        let text = import::presets::get(name).ok_or_else(|| anyhow::anyhow!("no preset '{name}'"))?;
        return Ok(Output::new("import")
            .unpaged()
            .item(serde_json::json!({"preset": name, "map": text}), text.trim_end().to_string()));
    }
    let map_path = a.map.clone().expect("clap requires --map without --preset");
    let s = session::open(g)?;
    let map = ImportMap::load(&session::arg_path(g, &map_path))?;
    let p = import::build(&s.corpus, &map)?;
    let unresolved = p.rows.iter().filter(|r| !r.unresolved.is_empty()).count();
    let mut o = Output::new("import")
        .field("rows", p.rows.len())
        .field("unresolved", unresolved)
        .field("aliases", p.alias_rows.len())
        .field("uploads", p.uploads.len())
        .head(format!(
            "{} source(s), {} alias row(s), {} need a decision, {} large file(s) for the store",
            p.rows.len(),
            p.alias_rows.len(),
            unresolved,
            p.uploads.len()
        ));
    for r in &p.rows {
        let text = row_text(r);
        o = o.item(to_value(r)?, text);
    }
    if !a.apply || !a.write || a.dry_run {
        let o = if a.apply { o.foot("preview only; nothing was written") } else { o };
        let max = s.corpus.cfg.raw.write.max_files;
        let n = super::preview::files(&p.plan) + alias_writes(&s, &p);
        let (o, _) = super::preview::bound(o, n, max, false, a.yes);
        let o = if unresolved > 0 { o.code(Code::Findings) } else { o };
        return Ok(before_write(&s, &p, o, &map_path.display().to_string()));
    }
    let Some(alias_file) = s.corpus.cfg.raw.aliases.file.clone() else {
        bail!("set [aliases] file = \".spec/_aliases.tsv\" in spec.toml before applying an import")
    };
    if !p.uploads.is_empty() && s.corpus.cfg.raw.files.store.is_none() {
        bail!("{} attachment(s) are over the repository limit; configure [files.store] first", p.uploads.len());
    }
    if p.rows.is_empty() && p.uploads.is_empty() && p.plan.changes.is_empty() {
        // C121: nothing to import writes nothing, not even an empty alias table.
        return Ok(o
            .field("written", false)
            .foot("no source documents matched the map; nothing was written")
            .next(format!("spec import --map {}", specfold_core::shell::word(map_path.display()))));
    }
    if unresolved > 0 && !a.force {
        // C155: a row still marked ! is not imported half-decided.
        let o = o.code(Code::Refused).head(format!(
            "{unresolved} row(s) marked ! still need a decision: nothing was written; settle them, or add --force"
        ));
        let o = before_write(&s, &p, o, &map_path.display().to_string());
        return Ok(o.next(format!(
            "spec import --map {} --apply --write --force",
            specfold_core::shell::word(map_path.display())
        )));
    }
    // C138: the alias table counts toward [write] max_files too.
    let n = super::preview::files(&p.plan) + alias_writes(&s, &p);
    let (o, go) = super::preview::bound(o, n, s.corpus.cfg.raw.write.max_files, true, a.yes);
    if !go {
        return Ok(o.next(format!(
            "spec import --map {} --apply --write --yes",
            specfold_core::shell::word(map_path.display())
        )));
    }
    let root = &s.corpus.cfg.root;
    let table_existed = root.join(&alias_file).exists();
    let mut o = o.field("written", true);
    let mut changes = vec![];
    for c in &p.plan.changes {
        use specfold_core::authoring::Change;
        let (op, path) = match c {
            Change::Write { path, created: true, .. } => ("create", path.clone()),
            Change::Write { path, .. } => ("update", path.clone()),
            Change::Move { from, to } => ("move", format!("{from} -> {to}")),
            Change::Copy { from, to } => ("copy", format!("{from} -> {to}")),
            Change::Remove { path } => ("remove", path.clone()),
        };
        o = o.foot(format!("{op:<7} {path}"));
        changes.push(serde_json::json!({"op": op, "path": path}));
    }
    let op = if table_existed { "update" } else { "create" };
    o = o.foot(format!("{op:<7} {alias_file}"));
    changes.push(serde_json::json!({"op": op, "path": alias_file}));
    let o = o.field("changes", changes);
    // The alias table is part of the write, so the undo takes it back too.
    let mut with_table = p.plan.clone();
    with_table.changes.push(specfold_core::authoring::Change::Write {
        path: alias_file.clone(),
        content: String::new(),
        created: !table_existed,
    });
    let saved = super::preview::keep_untracked(&p.plan, root)?;
    p.plan.apply(root)?;
    let table_path = s.corpus.cfg.root.join(&alias_file);
    let existing = std::fs::read_to_string(&table_path).unwrap_or_default();
    if let Some(dir) = table_path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&table_path, import::alias_table(&existing, &p.alias_rows))?;
    if let Some(st) = store::open(&s.corpus.cfg)? {
        for (path, sha) in &p.uploads {
            st.put(&s.corpus.cfg.root.join(path), sha).with_context(|| format!("uploading {path}"))?;
        }
    }
    let undo = super::preview::undo(&with_table, root, &saved);
    Ok(o.field("undo", undo.clone())
        .foot(format!("{} change(s) written", p.plan.changes.len() + 1))
        .foot(format!("undo: {undo}"))
        .foot(format!("imported; commit with SPECFOLD_IMPORT=1 so the alias table may grow ({alias_file})"))
        .next("spec check && spec fix --rule L1 --rule L2 --write"))
}

/// What to do before `--apply --write`: settle the decisions, then set the alias table, and only
/// then write. A status without a mapping is answered by a line in the map's `[status]` table.
fn before_write(s: &session::Session, p: &import::ImportPlan, o: Output, map: &str) -> Output {
    let mut statuses: Vec<String> = vec![];
    let mut other = 0;
    for r in &p.rows {
        for u in &r.unresolved {
            let old = u.strip_prefix("status '").and_then(|x| x.strip_suffix("' has no mapping"));
            match old {
                Some(old) => {
                    let ty = r.id.split(':').next().unwrap_or("");
                    let allowed = s.corpus.cfg.registry.get(ty).map(|t| t.statuses.clone()).unwrap_or_default();
                    let lower = old.to_lowercase();
                    let to = crate::commands::setup::help::nearest(&lower, allowed.iter().map(String::as_str))
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("<{}>", allowed.join("|")));
                    let line = format!("{} = \"{to}\"", toml_key(&lower));
                    if !statuses.contains(&line) {
                        statuses.push(line);
                    }
                }
                None => other += 1,
            }
        }
    }
    if !statuses.is_empty() {
        let mut o = o.foot(format!("add under [status] in {map}, then plan again:"));
        for l in &statuses {
            o = o.foot(format!("  {l}"));
        }
        return o.next(format!("{}  # under [status] in {map}", statuses[0]));
    }
    if other > 0 {
        return o
            .foot(format!("{other} line(s) marked ! need a decision; edit {map} or the sources, then plan again"))
            .next(format!("spec import --map {}", specfold_core::shell::word(map)));
    }
    if s.corpus.cfg.raw.aliases.file.is_none() {
        let toml = std::fs::read_to_string(s.corpus.cfg.root.join("spec.toml")).unwrap_or_default();
        let o = o.foot("applying needs [aliases] file in spec.toml: old names and links resolve through it");
        return if toml.lines().any(|l| l.trim() == "[aliases]") {
            o.next("file = \".spec/_aliases.tsv\"  # under [aliases] in spec.toml")
        } else {
            o.next("printf '\\n[aliases]\\nfile = \".spec/_aliases.tsv\"\\n' >> spec.toml")
        };
    }
    o.next(format!("spec import --map {} --apply --write", specfold_core::shell::word(map)))
}

/// 1 when applying would create or change the alias table, else 0.
fn alias_writes(s: &session::Session, p: &import::ImportPlan) -> usize {
    let Some(file) = &s.corpus.cfg.raw.aliases.file else { return 0 };
    let existing = std::fs::read_to_string(s.corpus.cfg.root.join(file)).ok();
    let new = import::alias_table(existing.as_deref().unwrap_or_default(), &p.alias_rows);
    usize::from(existing.as_deref() != Some(new.as_str()))
}

/// A status as a TOML key: bare when it can be, quoted otherwise.
fn toml_key(k: &str) -> String {
    if !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        k.to_string()
    } else {
        format!("{k:?}")
    }
}

/// One plan row as the review text shows it.
fn row_text(r: &import::Row) -> String {
    let mut text = format!("{:<8} {}\n         -> {} {}", r.kind, r.source, r.id, r.path);
    if !r.status_from.is_empty() || !r.status_to.is_empty() {
        text.push_str(&format!(
            "  status {} -> {}",
            if r.status_from.is_empty() { "(none)" } else { &r.status_from },
            r.status_to
        ));
    }
    if !r.owner_reason.is_empty() {
        text.push_str(&format!("\n         owner: {}", r.owner_reason));
    }
    for n in &r.notes {
        text.push_str(&format!("\n         note: {n}"));
    }
    for u in &r.unresolved {
        text.push_str(&format!("\n         ! {u}"));
    }
    text
}
