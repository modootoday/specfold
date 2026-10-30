//! `spec export`: the built-in Markdown tree, or the graph handed to an exporter plugin.

use crate::cli::args::{global::Global, setup::ExportArgs};
use crate::commands::outbound;
use crate::output::{Code, Output};
use crate::session;
use anyhow::{Result, anyhow, bail};
use serde_json::{Value, json};
use specfold_core::export::{self, markdown, payload::Wants};
use specfold_core::plugins::{self, payload, process};

/// Runs the built-in secrets check and the public guards over what is about to leave; the hits
/// that let it through are added to `o` either way.
fn guard(
    cfg: &specfold_core::Config,
    a: &ExportArgs,
    blobs: &[(String, Vec<u8>)],
    sources: &[(String, Vec<u8>)],
    o: Output,
) -> Result<(Output, bool)> {
    if a.no_guard {
        return Ok((o.foot("secrets check and public guards skipped (--no-guard)"), false));
    }
    let mut gate = outbound::gate(cfg, |g| g.public, blobs, &[])?;
    // The built-in secrets check runs unless a public guard already reads secrets, over the
    // source files so a hit names the line to fix; a line already reported is not repeated.
    let covered = cfg.raw.guards.iter().any(|g| g.public && g.secrets);
    let builtin = if covered { vec![] } else { specfold_core::guards::builtin_secrets(sources) };
    for (g, f) in builtin {
        if !gate.hits.iter().any(|(_, h, _)| h.path == f.path && h.line == f.line) {
            gate.hits.push((g, f, false));
        }
    }
    let mut o = o.field("guard", gate.json());
    for l in gate.lines() {
        o = o.head(l);
    }
    if gate.refuses() {
        // `spec guard` re-runs a configured guard; `spec guard secrets` the built-in check.
        let refusing_guard = gate.refusing().iter().any(|g| g != "secrets");
        let n = gate.hits.iter().filter(|(_, f, _)| f.severity == specfold_core::check::Severity::Error).count();
        return Ok((
            o.head(format!(
                "nothing exported: {} found {n} hit(s); remove the value, or pass --no-guard to skip the check",
                gate.refusing().join(", ")
            ))
            .code(Code::Findings)
            .next(if refusing_guard { "spec guard --range all" } else { "spec guard secrets" }),
            true,
        ));
    }
    Ok((o, false))
}

/// One line naming the exported documents' attachments the secrets check could not read.
fn unscanned(c: &specfold_core::corpus::Corpus, only: &[usize]) -> Option<String> {
    let max = c.cfg.raw.files.scan_max_bytes;
    let mut skipped: Vec<String> = only
        .iter()
        .flat_map(|&i| specfold_core::guards::secrets::attachment_scan(&c.cfg.root, &c.docs[i], max).1)
        .collect();
    skipped.sort();
    skipped.dedup();
    specfold_core::guards::secrets::skipped_note(&c.cfg.root, &skipped, max)
}

/// The files the exported documents come from (main file and chapters) and their repository
/// text attachments up to [files] scan_max_bytes (the set rule U2 reads), as guard blobs.
fn source_blobs(c: &specfold_core::corpus::Corpus, only: &[usize]) -> Vec<(String, Vec<u8>)> {
    let mut blobs = vec![];
    let max = c.cfg.raw.files.scan_max_bytes;
    for &i in only {
        let d = &c.docs[i];
        let attachments = specfold_core::guards::secrets::scanned_attachments(&c.cfg.root, d, max);
        for path in std::iter::once(&d.path).chain(&d.chapters).chain(&attachments) {
            if let Ok(bytes) = std::fs::read(c.cfg.root.join(path)) {
                blobs.push((path.clone(), bytes));
            }
        }
    }
    blobs
}

pub fn run(g: &Global, a: &ExportArgs) -> Result<Output> {
    if a.plugin == export::BUILT_IN {
        return built_in(g, a);
    }
    for (set, flag) in [(a.target.is_some(), "--target"), (a.banner, "--banner"), (a.fail_on.is_some(), "--fail-on")] {
        if set {
            bail!("{flag} belongs to the built-in exporter: spec export markdown {flag} …");
        }
    }
    let s = session::open(g)?;
    let loaded = plugins::load(&s.corpus.cfg);
    let p = loaded.iter().find(|p| p.def.name == a.plugin).ok_or_else(|| anyhow!("no plugin named '{}'", a.plugin))?;
    let d = p.description.as_ref().map_err(|e| anyhow!("{e}"))?;
    if d.provides.exporters.is_empty() {
        bail!("plugin {} provides no exporter", a.plugin);
    }
    let format = a.format.clone().unwrap_or_else(|| d.provides.exporters[0].clone());
    let wants = Wants::from(&d.wants);
    // Local documents stay out unless asked for, so private notes never reach a plugin.
    let only = export::payload::visible(&s.corpus, a.local);
    let docs = if wants.any() {
        export::payload::docs(&s.corpus, &s.graph, wants, g.max_bytes, &only)
    } else {
        payload::docs(&s.corpus, Some(&only))
    };
    let params = json!({
        "format": format,
        "out": a.out.as_ref().map(|p| session::arg_path(g, p).display().to_string()),
        "docs": docs,
        "edges": export::payload::edges(&s.corpus, &s.graph, &only),
    });
    let o = Output::new("export").unpaged().field("plugin", a.plugin.clone()).field("format", format.clone());
    // The plugin gets each document's text, so the guard reads the files it comes from.
    let blobs = source_blobs(&s.corpus, &only);
    let (o, refused) = guard(&s.corpus.cfg, a, &blobs, &blobs, o)?;
    if refused {
        return Ok(o);
    }
    let o = match (a.no_guard, unscanned(&s.corpus, &only)) {
        (false, Some(note)) => o.field("not_scanned", note.clone()).foot(note),
        _ => o,
    };
    // C114: a preview never runs the plugin, since the plugin is what sends.
    if a.dry_run {
        let n = docs.as_array().map_or(0, Vec::len);
        return Ok(o
            .field("written", false)
            .field("docs", n)
            .head(format!("would send {n} document(s) to {} ({format}): nothing was sent", a.plugin))
            .next(format!("spec export {} --format {format}", a.plugin)));
    }
    let result: Value = process::call(&s.corpus.cfg, &p.def, "export.run", params)?;
    Ok(o.field("result", result).head(format!("exported with {} ({format})", a.plugin)))
}

fn built_in(g: &Global, a: &ExportArgs) -> Result<Output> {
    let names = markdown::Target::ALL.join(", ");
    let first = markdown::Target::ALL[0];
    let out_word = a.out.as_ref().map_or("export".to_string(), |p| session::shell_word(&p.display().to_string()));
    let Some(t) = &a.target else {
        bail!(
            "spec export markdown needs --target ({names}); next: spec export markdown --target {first} --out {out_word}"
        )
    };
    let target = markdown::Target::parse(t).ok_or_else(|| {
        anyhow!("unknown --target '{t}' (one of {names}); next: spec export markdown --target {first} --out {out_word}")
    })?;
    let Some(out) = &a.out else {
        bail!(
            "spec export markdown needs --out <dir>; next: spec export markdown --target {} --out export",
            target.name()
        )
    };
    let strict = match a.fail_on.as_deref() {
        None => false,
        Some("unresolved") => true,
        Some(x) => bail!("unknown --fail-on '{x}' (only: unresolved)"),
    };
    if a.format.is_some() {
        bail!("--format is for plugins; the built-in exporter takes --target");
    }
    let s = session::open(g)?;
    // C161: read and written from -C; shown as typed, so the next commands keep working.
    let out_path = session::arg_path(g, out);
    let previous = markdown::check_out(&out_path, &s.corpus.cfg.root).map_err(|e| {
        let m = e.to_string();
        if !(m.contains("not empty") || m.contains("not a directory")) {
            return e;
        }
        // A fresh directory beside the refused one is accepted.
        let fresh = session::shell_word(&format!("{}-export", out.display().to_string().trim_end_matches('/')));
        anyhow!("{m}; next: spec export markdown --target {} --out {fresh}", target.name())
    })?;
    let opts = markdown::Options { target, local: a.local, banner: a.banner };
    let tree = markdown::build(&s.corpus, &s.graph, &opts);
    let n = tree.unresolved.len();
    let mut o = Output::new("export")
        .unpaged()
        .field("plugin", export::BUILT_IN)
        .field("target", target.name())
        .field("out", out.display().to_string())
        .field("docs", tree.docs)
        .field("files", tree.files.len())
        .field("unresolved", json!(tree.unresolved));
    for u in &tree.unresolved {
        o = o.item(json!(u), format!("{}: {} written as text ({})", u.path, u.raw, u.reason));
    }
    if strict && n > 0 {
        return Ok(o
            .head(format!("nothing written: {n} reference(s) lead nowhere in the tree"))
            .code(Code::Findings)
            .next("spec check --rule L1"));
    }
    let blobs: Vec<(String, Vec<u8>)> = tree.files.iter().map(|(p, t)| (p.clone(), t.as_bytes().to_vec())).collect();
    let visible = export::payload::visible(&s.corpus, a.local);
    let sources = source_blobs(&s.corpus, &visible);
    let (o, refused) = guard(&s.corpus.cfg, a, &blobs, &sources, o)?;
    if refused {
        return Ok(o);
    }
    // Attachments the secrets check could not read leave too: say so, as check and guard do.
    let o = match (a.no_guard, unscanned(&s.corpus, &visible)) {
        (false, Some(note)) => o.field("not_scanned", note.clone()).foot(note),
        _ => o,
    };
    // Replacing an earlier export overwrites files outside the repository: --yes confirms it (CLI-3).
    let again = session::shell_word(&out.display().to_string());
    // C114/C115: a preview lists what would be written and replaced, writes nothing, exits 0.
    if a.dry_run {
        let mut o = o.field("written", false).field("replaces", json!(previous)).head(format!(
            "would export {} document(s) for {} into {} ({} files){}: nothing was written",
            tree.docs,
            target.name(),
            out.display(),
            tree.files.len(),
            if previous.is_empty() {
                String::new()
            } else {
                format!(", replacing an earlier export ({} file(s))", previous.len())
            }
        ));
        for p in tree.files.keys().take(20) {
            o = o.foot(format!("write  {p}"));
        }
        let yes = if previous.is_empty() { "" } else { " --yes" };
        return Ok(o.next(format!("spec export markdown --target {} --out {again}{yes}", target.name())));
    }
    if !previous.is_empty() && !a.yes {
        return Ok(o
            .code(crate::output::Code::Refused)
            .field("written", false)
            .field("replaces", json!(previous))
            .head(format!(
                "{} holds an earlier export ({} file(s)) that this one replaces: nothing was written; add --yes to confirm",
                out.display(),
                previous.len()
            ))
            .next(format!("spec export markdown --target {} --out {again} --yes", target.name())));
    }
    markdown::write(&out_path, &tree, target, &previous)?;
    let mut o = o.head(format!(
        "exported {} document(s) for {} into {} ({} files)",
        tree.docs,
        target.name(),
        out.display(),
        tree.files.len()
    ));
    if n > 0 {
        o = o.foot(format!("{n} reference(s) written as plain text")).next("spec check --rule L1");
    } else {
        // Done: look at what changed in the output rather than exporting again.
        // C104: git sees only an --out inside the repository.
        let inside = std::fs::canonicalize(&out_path)
            .ok()
            .zip(std::fs::canonicalize(&s.corpus.cfg.root).ok())
            .is_some_and(|(o, r)| o.starts_with(r));
        o = if inside { o.next(format!("git status --short -- {again}")) } else { o.next(format!("ls {again}")) };
    }
    Ok(o)
}
