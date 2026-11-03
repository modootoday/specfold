//! `spec new`.

use super::preview;
use crate::cli::args::{global::Global, write::NewArgs};
use crate::output::Output;
use crate::session;
use anyhow::{Context, Result, bail};
use serde_json::json;
use specfold_core::assets;
use specfold_core::authoring::new::{self, NewDoc};
use specfold_core::config::types::AssetKind;
use specfold_core::model::stamp::slugify;

/// CLI-1: `new` previews unless --write (--dry-run previews too), as every write command does.
pub fn run(g: &Global, a: &NewArgs) -> Result<Output> {
    create(g, a)
}

/// The same `spec new` with --write: the next step of a preview.
fn rerun(a: &NewArgs) -> String {
    use specfold_core::suggest::shell::{opt, word};
    let mut cmd = String::from("spec new");
    if let Some(ty) = &a.ty {
        cmd.push_str(&format!(" {}", word(ty)));
    }
    let flags = [("--title", a.title.as_ref().or(a.title_text.as_ref()))]
        .into_iter()
        .chain([("--owner", a.owner.as_ref()), ("--slug", a.slug.as_ref()), ("--from", a.from.as_ref())])
        .chain([("--translation", a.translation.as_ref()), ("--description", a.description.as_ref())]);
    for (flag, v) in flags {
        if let Some(v) = v {
            cmd.push_str(&format!(" {}", opt(flag, v)));
        }
    }
    if let Some(n) = a.number {
        cmd.push_str(&format!(" --number {n}"));
    }
    for (flag, vs) in [("--governs", &a.governs), ("--depends-on", &a.depends_on)] {
        for v in vs {
            cmd.push_str(&format!(" {}", opt(flag, v)));
        }
    }
    if let Some(p) = &a.depends_on_file {
        cmd.push_str(&format!(" {}", opt("--depends-on-file", p)));
    }
    for (flag, on) in [(" --local", a.local), (" --bundle", a.bundle)] {
        if on {
            cmd.push_str(flag);
        }
    }
    cmd.push_str(" --write");
    cmd
}

fn create(g: &Global, a: &NewArgs) -> Result<Output> {
    // C160: a title is one line; a line break splits the heading and list rows.
    if [&a.title, &a.title_text].into_iter().flatten().any(|t| t.contains(session::LINE_BREAKS)) {
        let ty = a.ty.as_deref().unwrap_or("<type>");
        bail!(
            "the title has a line break; give it on one line; next: spec new {ty} --title \"<one-line title>\" --write"
        );
    }
    let s = session::open(g)?;
    if let Some(lang) = &a.translation {
        return translation(&s, a, lang);
    }
    let ty = a.ty.clone().unwrap_or_default();
    if let Some(kind) = asset_kind(&s, &ty) {
        return asset(&s, a, kind);
    }
    let governs: Vec<String> =
        a.governs.iter().map(|p| s.loc.governed_path(&s.corpus.cfg, p)).collect::<Result<_>>()?;
    let mut req = NewDoc {
        ty: ty.clone(),
        title: a.title().to_string(),
        governs,
        owner: a.owner.clone(),
        slug: a.slug.clone(),
        from: a.from.clone(),
        local: a.local,
        bundle: a.bundle,
        key: None,
        number: a.number,
        body: None,
        depends_on: depends_on(a)?,
        here: s.loc.here(&s.corpus.cfg),
    };
    let (mut created, mut plan) = new::plan(&s.corpus, &s.graph, &req)?;
    // C206: a timestamp or number id is reserved with create-new before the write; a clash with
    // a run in another process takes the next key. A --number the author picked is not moved.
    let kind = s.corpus.cfg.registry.get(&ty).map(|t| t.key).filter(|_| a.number.is_none());
    for _ in 0..120 {
        let key = created.id.rsplit('/').next().unwrap_or_default().to_string();
        let next = kind.and_then(|k| k.next_after(&key));
        if !a.write || next.is_none() || reserve(&s.corpus.cfg.spec_home, &created.id, &created.path, &key) {
            break;
        }
        req.key = next;
        (created, plan) = new::plan(&s.corpus, &s.graph, &req)?;
    }
    let o = Output::new("new")
        .unpaged()
        .field("id", created.id.clone())
        .field("path", created.path.clone())
        .field("owner_reason", created.owner_reason.clone())
        .head(format!("{} {}  {}", if a.write { "created" } else { "would create" }, created.id, created.path))
        .head(format!("owner: {}", created.owner_reason))
        .head("dates and timestamp keys are UTC");
    let o = match &created.slug_note {
        Some(note) => o.field("slug_note", note.clone()).head(format!("warn  {note}")),
        None => o,
    };
    let o = match supersede_note(&s, a, &created) {
        Some(note) => o.field("supersede_note", note.clone()).head(note),
        None => o,
    };
    // The same title twice is usually a rerun; say so before a second copy is written.
    let same: Vec<String> = s
        .corpus
        .docs
        .iter()
        .filter(|d| {
            d.id.ty == ty && d.str_field("title").is_some_and(|t| t.trim().eq_ignore_ascii_case(a.title().trim()))
        })
        .map(|d| d.id.to_string())
        .collect();
    let o = if same.is_empty() {
        o
    } else {
        o.field("same_title", json!(same)).head(format!("warn  same title as {}", same.join(", ")))
    };
    // C248: the exclude goes in before the write, so the undo is the `rm -f` of an ignored file.
    let ignored = if a.local && a.write {
        ignore_local(&s.corpus.cfg.spec_home, &created.path, s.corpus.cfg.local_dir())
    } else {
        None
    };
    let mut o = preview::finish(o, &plan, &s.corpus.cfg.spec_home, a.write, &rerun(a))?;
    if let Some(note) = ignored {
        o = o.field("ignored", note.clone()).foot(note);
    }
    // A slug taken from a few ASCII letters of a mostly non-ASCII title rarely names the
    // document: the next step is to name it.
    Ok(match (&created.slug_note, !a.write) {
        (Some(_), false) => o.next(format!(
            "spec mv {} --slug <kebab-case-name> --write",
            specfold_core::suggest::shell::word(&created.id)
        )),
        (Some(_), true) => o.next("rerun with --slug <kebab-case-name>"),
        (None, true) => o,
        (None, false) => o.next(format!("spec show {}", specfold_core::suggest::shell::word(&created.id))),
    })
}

/// With --from on a type that starts pending (a draft), the old document stays in force until the
/// new one holds; says so and how.
fn supersede_note(s: &session::Session, a: &NewArgs, created: &new::Created) -> Option<String> {
    let from = a.from.as_deref()?;
    let ty = s.corpus.cfg.registry.get(created.id.split(':').next()?)?;
    if ty.in_force(&ty.initial) {
        return None;
    }
    let old = &s.corpus.docs[session::resolve(s, from).ok()?];
    let to = specfold_core::authoring::takes_effect::effective_status(ty).unwrap_or("active");
    Some(format!(
        "{} stays {} until {} is {to}; spec set {} status={to} --write marks it superseded",
        old.id,
        old.status().unwrap_or_default(),
        created.id,
        specfold_core::suggest::shell::word(&created.id)
    ))
}

/// Takes `id` for this process: false when another `spec new` holds it or already wrote a file
/// with that key next to `path`.
fn reserve(root: &std::path::Path, id: &str, path: &str, key: &str) -> bool {
    let git = root.join(".git");
    let base = if git.is_dir() { git } else { std::env::temp_dir() };
    let dir = base.join("specfold-ids");
    if std::fs::create_dir_all(&dir).is_err() {
        return true;
    }
    // Old reservations are of no use once their files are written.
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let old = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok());
            if old.is_some_and(|d| d.as_secs() > 3600) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let name: String = id.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    let took = std::fs::OpenOptions::new().write(true).create_new(true).open(dir.join(name)).is_ok();
    if !took {
        return false;
    }
    let mut parent = root.join(path);
    parent.pop();
    let mut dirs = vec![parent.clone()];
    if let Some(up) = parent.parent() {
        dirs.push(up.to_path_buf());
    }
    !dirs.iter().any(|d| {
        std::fs::read_dir(d)
            .is_ok_and(|rd| rd.flatten().any(|e| e.file_name().to_string_lossy().starts_with(&format!("{key}-"))))
    })
}

/// `spec new --translation <lang> --from <source>`.
fn translation(s: &session::Session, a: &NewArgs, lang: &str) -> Result<Output> {
    let from = a.from.as_deref().unwrap_or_default();
    let title = a.title.as_deref().or(a.title_text.as_deref());
    let (created, plan) = new::translation(&s.corpus, &s.graph, from, lang, title)?;
    if let Some(ty) = &a.ty
        && !created.id.starts_with(&format!("{ty}:"))
    {
        bail!("--from {from} is not a {ty}; leave out the type to translate it as it is");
    }
    let o = Output::new("new")
        .unpaged()
        .field("id", created.id.clone())
        .field("path", created.path.clone())
        .field("owner_reason", created.owner_reason.clone())
        .head(format!("{} {}  {}", if a.write { "created" } else { "would create" }, created.id, created.path))
        .head(created.owner_reason.clone())
        .head("the body is the source's: translate it; source_rev already records the source it was copied from");
    let o = preview::finish(o, &plan, &s.corpus.cfg.spec_home, a.write, &rerun(a))?;
    Ok(if !a.write { o } else { o.next(format!("spec show {}", specfold_core::suggest::shell::word(&created.id))) })
}

/// The local directory is meant to stay out of git; the first local document makes sure it does,
/// with a line in .git/info/exclude (which is never committed) when nothing ignores it yet.
fn ignore_local(root: &std::path::Path, path: &str, local_dir: &str) -> Option<String> {
    let git = |args: &[&str]| std::process::Command::new("git").arg("-C").arg(root).args(args).output().ok();
    let checked = git(&["check-ignore", "-q", "--no-index", path])?;
    if checked.status.code() != Some(1) {
        return None; // ignored already, or not a git repository
    }
    let out = git(&["rev-parse", "--git-path", "info/exclude"])?;
    let rel = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() || rel.is_empty() {
        return None;
    }
    let file = root.join(&rel);
    let name = local_dir.trim_end_matches('/').rsplit('/').next().unwrap_or(local_dir);
    let mut text = std::fs::read_to_string(&file).unwrap_or_default();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&format!("# local documents (spec new --local)\n{name}/\n"));
    std::fs::create_dir_all(file.parent()?).ok()?;
    std::fs::write(&file, text).ok()?;
    Some(format!("{name}/ was not ignored by git; added it to {rel}"))
}

/// skill, rule and agent make agent assets unless a document type of that name is declared.
fn asset_kind(s: &session::Session, ty: &str) -> Option<AssetKind> {
    if s.corpus.cfg.registry.get(ty).is_some() {
        return None;
    }
    match ty {
        "skill" => Some(AssetKind::Skill),
        "rule" => Some(AssetKind::Rule),
        "agent" => Some(AssetKind::Agent),
        _ => None,
    }
}

fn asset(s: &session::Session, a: &NewArgs, kind: AssetKind) -> Result<Output> {
    let name = a.slug.clone().unwrap_or_else(|| slugify(a.title()));
    // C147: an empty or path-like name would write SKILL.md straight into the asset folder.
    if !specfold_core::model::stamp::is_slug(&name) {
        if a.slug.is_some() {
            bail!("--slug '{name}' is not a kebab-case name (a-z, 0-9, '-'); rerun with --slug my-{}", kind.as_str());
        }
        bail!(
            "the title has no ASCII words to make a {} name from; rerun with --slug my-{} (an English kebab-case name)",
            kind.as_str(),
            kind.as_str()
        );
    }
    let description = a.description.clone().unwrap_or_else(|| a.title().to_string());
    let cfg = &s.corpus.cfg;
    let Some((path, text, covered)) = assets::template(cfg, kind, &name, &description, a.title()) else {
        bail!("no template for {}", kind.as_str());
    };
    let full = cfg.spec_home.join(&path);
    if full.exists() {
        bail!("{path} already exists; edit it instead");
    }
    let mut o = Output::new("new")
        .unpaged()
        .field("kind", kind.as_str())
        .field("name", name.clone())
        .field("declared", covered)
        .field("path", path.clone())
        .head(format!("{} {} {name}  {path}", if a.write { "created" } else { "would create" }, kind.as_str()))
        .head("agent assets carry no id");
    if !covered {
        o = o.head(format!(
            "not declared in spec.toml, so check skips it: add [[assets]] kind = \"{}\" path = \"{}\"",
            kind.as_str(),
            declare_path(kind, &path)
        ));
    }
    if !a.write {
        return Ok(o.field("text", text).foot("preview only; nothing was written").next(rerun(a)).preview());
    }
    if let Some(dir) = full.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    std::fs::write(&full, text).with_context(|| format!("cannot write {path}"))?;
    Ok(o.field("written", true).next("spec check --rule A1"))
}

/// The directory to declare for a new asset path.
fn declare_path(kind: AssetKind, path: &str) -> String {
    let up = if kind == AssetKind::Skill { 2 } else { 1 };
    let mut p = path;
    for _ in 0..up {
        p = p.rsplit_once('/').map_or("", |(d, _)| d);
    }
    p.to_string()
}

/// E6: `--depends-on` values (comma lists already split) and the ids `--depends-on-file` lists.
fn depends_on(a: &NewArgs) -> Result<Vec<String>> {
    let mut ids: Vec<String> = a.depends_on.iter().map(|v| v.trim().to_string()).filter(|v| !v.is_empty()).collect();
    if let Some(p) = &a.depends_on_file {
        let text = if p == "-" {
            std::io::read_to_string(std::io::stdin())?
        } else {
            std::fs::read_to_string(p).map_err(|e| {
                anyhow::anyhow!("cannot read --depends-on-file {p}: {e}; next: list one id per line in that file")
            })?
        };
        let lines = text.lines().map(|l| l.split('#').next().unwrap_or("")).flat_map(|l| l.split(','));
        ids.extend(lines.map(str::trim).filter(|l| !l.is_empty()).map(str::to_string));
    }
    let mut seen = std::collections::HashSet::new();
    ids.retain(|i| seen.insert(i.clone()));
    Ok(ids)
}
