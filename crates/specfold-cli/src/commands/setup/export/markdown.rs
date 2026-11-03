//! `spec export markdown`: the built-in exporter writing a Markdown tree for one target.

use super::flags::{content_flags, kept_flags};
use super::gate::{guard, source_blobs, unscanned};
use crate::cli::args::{global::Global, setup::ExportArgs};
use crate::output::{Code, Output};
use crate::session;
use anyhow::{Result, anyhow, bail};
use serde_json::json;
use specfold_core::export::{self, markdown};

pub(super) fn built_in(g: &Global, a: &ExportArgs) -> Result<Output> {
    let (target, out, strict) = built_in_args(a)?;
    let kept = kept_flags(a);
    let s = session::open(g)?;
    // C161: read and written from -C; shown as typed, so the next commands keep working.
    let out_path = s.loc.arg_path(out);
    let previous = markdown::check_out(&out_path, &s.loc.cwd.join(&out_path), &s.corpus.cfg.root).map_err(|e| {
        let m = e.to_string();
        if !(m.contains("not empty") || m.contains("not a directory")) {
            return e;
        }
        // A fresh directory beside the refused one is accepted.
        let fresh = specfold_core::shell::quote(&format!("{}-export", out.display().to_string().trim_end_matches('/')));
        anyhow!("{m}; next: spec export markdown --target {} --out {fresh}{kept}", target.name())
    })?;
    let opts = markdown::Options { target, local: a.local, banner: a.banner };
    export::payload::readable(&s.corpus, &export::payload::visible(&s.corpus, a.local))?;
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
    if let Some(o) = foreign_files(o.clone(), &out_path, &tree, &previous, out, target.name()) {
        return Ok(o);
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
    let again = specfold_core::shell::quote(&out.display().to_string());
    if a.dry_run {
        return Ok(dry_run(o, a, &tree, &previous, out, target, &again));
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
            .next(format!("spec export markdown --target {} --out {again}{} --yes", target.name(), content_flags(a))));
    }
    markdown::write(&out_path, &tree, target, &previous)?;
    let o = o.head(format!(
        "exported {} document(s) for {} into {} ({} files)",
        tree.docs,
        target.name(),
        out.display(),
        tree.files.len()
    ));
    Ok(done_next(o, tree.docs, n, &out_path, &s.corpus.cfg.root, &again))
}

/// The built-in exporter's own flags: the target, the output folder, and whether an
/// unresolved reference fails the export.
fn built_in_args(a: &ExportArgs) -> Result<(markdown::Target, &std::path::Path, bool)> {
    let kept = kept_flags(a);
    let names = markdown::Target::ALL.join(", ");
    let first = markdown::Target::ALL[0];
    let out_word =
        a.out.as_ref().map_or("export".to_string(), |p| specfold_core::shell::quote(&p.display().to_string()));
    let Some(t) = &a.target else {
        bail!(
            "spec export markdown needs --target ({names}); next: spec export markdown --target {first} --out {out_word}{kept}"
        )
    };
    let target = markdown::Target::parse(t).ok_or_else(|| {
        anyhow!("unknown --target '{t}' (one of {names}); next: spec export markdown --target {first} --out {out_word}{kept}")
    })?;
    let Some(out) = a.out.as_deref() else {
        bail!(
            "spec export markdown needs --out <dir>; next: spec export markdown --target {} --out export{kept}",
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
    Ok((target, out, strict))
}

/// C114/C115: a preview lists what would be written and replaced, writes nothing, exits 0.
fn dry_run(
    o: Output,
    a: &ExportArgs,
    tree: &markdown::Tree,
    previous: &[String],
    out: &std::path::Path,
    target: markdown::Target,
    again: &str,
) -> Output {
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
    let content = content_flags(a);
    o.next(format!("spec export markdown --target {} --out {again}{content}{yes}", target.name()))
}

/// C251: a file the export would write over that no earlier export listed is the user's own:
/// refused (exit 2, preview included) and named, so nothing of theirs is overwritten.
fn foreign_files(
    o: Output,
    out_path: &std::path::Path,
    tree: &markdown::Tree,
    previous: &[String],
    out: &std::path::Path,
    target: &str,
) -> Option<Output> {
    let theirs: Vec<&String> =
        tree.files.keys().filter(|p| !previous.contains(p) && out_path.join(p.as_str()).exists()).collect();
    if theirs.is_empty() {
        return None;
    }
    let mut o = o.code(Code::Usage).field("written", false).field("not_exported", json!(theirs)).head(format!(
        "{} holds {} file(s) no earlier export wrote, at paths this export writes: nothing was written",
        out.display(),
        theirs.len()
    ));
    for p in &theirs {
        o = o.foot(format!("keep   {p}  (not from an export; move it or export elsewhere)"));
    }
    let fresh = specfold_core::shell::quote(&format!("{}-export", out.display().to_string().trim_end_matches('/')));
    Some(o.next(format!("spec export markdown --target {target} --out {fresh}")))
}

/// What to run after a written export.
fn done_next(
    mut o: Output,
    docs: usize,
    n: usize,
    out_path: &std::path::Path,
    root: &std::path::Path,
    again: &str,
) -> Output {
    if docs == 0 {
        // C222: an empty repository exports the manifest alone.
        o = o
            .foot("no documents to export yet; the folder holds only the manifest")
            .next("spec new sot --title <TITLE> --write");
    } else if n > 0 {
        o = o.foot(format!("{n} reference(s) written as plain text")).next("spec check --rule L1");
    } else {
        // Done: look at what changed in the output rather than exporting again.
        // C104: git sees only an --out inside the repository.
        let inside = std::fs::canonicalize(out_path)
            .ok()
            .zip(std::fs::canonicalize(root).ok())
            .is_some_and(|(o, r)| o.starts_with(r));
        o = if inside { o.next(format!("git status --short -- {again}")) } else { o.next(format!("ls {again}")) };
    }
    o
}
