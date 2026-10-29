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
    let map = ImportMap::load(&map_path)?;
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
        o = o.item(to_value(r)?, text);
    }
    if !a.apply {
        return Ok(o
            .code(if unresolved > 0 { Code::Findings } else { Code::Ok })
            .next(format!("spec import --map {} --apply --write", map_path.display())));
    }
    if !a.write {
        return Ok(o
            .foot("preview only; nothing was written")
            .next(format!("spec import --map {} --apply --write", map_path.display())));
    }
    let Some(alias_file) = s.corpus.cfg.raw.aliases.file.clone() else {
        bail!("set [aliases] file = \".spec/_aliases.tsv\" in spec.toml before applying an import")
    };
    if !p.uploads.is_empty() && s.corpus.cfg.raw.files.store.is_none() {
        bail!("{} attachment(s) are over the repository limit; configure [files.store] first", p.uploads.len());
    }
    p.plan.apply(&s.corpus.cfg.root)?;
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
    Ok(o.foot(format!("imported; commit with SPECFOLD_IMPORT=1 so the alias table may grow ({alias_file})"))
        .next("spec check && spec fix --rule L1 --rule L2 --write"))
}
