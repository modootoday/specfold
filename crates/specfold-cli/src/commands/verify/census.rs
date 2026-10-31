//! `spec census`.

use crate::cli::args::{global::Global, verify::CensusArgs};
use crate::output::{Code, Output};
use crate::session;
use anyhow::Result;
use serde_json::to_value;
use specfold_core::census;
use specfold_core::translation::{self, State};

pub fn run(g: &Global, a: &CensusArgs) -> Result<Output> {
    let s = session::open(g)?;
    let c = census::take(&s.corpus, &s.graph);
    let mut o = Output::new("census").unpaged();
    if let serde_json::Value::Object(m) = to_value(&c)? {
        o.summary.extend(m);
    }
    if a.translations {
        return translations(&s, &a.lang, &a.fail_on);
    }
    if a.legacy {
        let gone = c.legacy_paths == 0 && c.strays == 0 && c.links_via_alias == 0 && c.path_cites == 0;
        o = o.head(format!(
            "legacy paths {} · strays {} · alias references {} · path citations {}",
            c.legacy_paths, c.strays, c.links_via_alias, c.path_cites
        ));
        return Ok(if gone { o.foot("the old layout is gone") } else { o.code(Code::Findings).next("spec check") });
    }
    o = o.head(format!("{} documents in {} spec directories · {} strays", c.docs, c.owners_with_docs, c.strays));
    o = o.head(format!(
        "references {} (broken {}, via alias {}) · path citations {}",
        c.links, c.links_broken, c.links_via_alias, c.path_cites
    ));
    o = o.head(format!(
        "attachments {} ({} bytes) · aliases {} · legacy paths {}",
        c.attachments, c.attachment_bytes, c.aliases, c.legacy_paths
    ));
    for (t, n) in &c.by_type {
        o = o.foot(format!("  {t:<10} {n:>6}"));
    }
    Ok(o.next("spec audit"))
}

fn translations(s: &session::Session, langs: &[String], fail_on: &str) -> Result<Output> {
    let r = translation::report(&s.corpus, &s.graph, langs);
    let mut o = Output::new("census");
    if let serde_json::Value::Object(m) = to_value(&r)? {
        o.summary.extend(m);
    }
    let count = |st: State| r.translations.iter().filter(|t| t.state == st).count();
    let missing: usize = r.missing.values().map(Vec::len).sum();
    let (behind, unrecorded, gone) = (count(State::Behind), count(State::Unrecorded), count(State::NoSource));
    o = o.head(format!(
        "{} translations of {} sources · current {} · behind {behind} · no source_rev {unrecorded} · no source {gone} · missing {missing}",
        r.translations.len(),
        r.sources,
        count(State::Current)
    ));
    for t in r.translations.iter().filter(|t| t.state != State::Current) {
        let what = match t.state {
            State::Behind => "behind",
            State::Unrecorded => "no source_rev",
            _ => "no source",
        };
        let changed =
            if t.sections.is_empty() { String::new() } else { format!(" in {}", translation::describe(&t.sections)) };
        // C212: the rows are the items, so --jsonl streams them and --limit/--offset page them.
        o = o.item(
            to_value(t)?,
            format!("  {:<14} {} ({}) → {}{changed}", what, t.id, t.lang.as_deref().unwrap_or("-"), t.source),
        );
    }
    for (lang, ids) in &r.missing {
        for id in ids {
            o = o.item(
                serde_json::json!({"id": id, "lang": lang, "state": "missing"}),
                format!("  {:<14} {id} has no {lang} translation", "missing"),
            );
        }
    }
    if behind + unrecorded + gone + missing == 0 {
        return Ok(o.foot("every translation is current"));
    }
    let next = r
        .translations
        .iter()
        .find(|t| matches!(t.state, State::Behind | State::Unrecorded))
        .map(|t| {
            format!(
                "spec set {} --source-rev --write  # after updating the translation",
                specfold_core::shell::word(&t.id)
            )
        })
        .unwrap_or_else(|| "spec check --rule R1 --rule L3".to_string());
    let fails = match fail_on {
        "none" => false,
        "stale" => behind + unrecorded + gone > 0,
        _ => true,
    };
    Ok(if fails { o.code(Code::Findings) } else { o }.next(next))
}
