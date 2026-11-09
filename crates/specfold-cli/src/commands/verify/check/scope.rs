//! What a `spec check` run covers: the files named, the rules named, standard input and the
//! commit range.

use crate::cli::args::verify::CheckArgs;
use crate::session;
use anyhow::{Result, bail};
use specfold_core::check::{self, Range};
use specfold_core::paths;

/// `--fail-on` must name rules that exist, so a gate cannot silently wait on a rule that never fires.
pub(super) fn check_fail_on(names: &[String]) -> Result<()> {
    let known: Vec<&str> = check::rules::catalog().iter().map(|r| r.info().id).collect();
    for n in names {
        if n != "any" && n != "none" && !n.starts_with("x.") && !known.contains(&n.as_str()) {
            bail!("--fail-on {n}: no such rule (spec explain lists them)");
        }
    }
    Ok(())
}

/// C132: `--rule`/`--skip` name rules that exist, so a typo cannot turn a check into a pass.
pub(crate) fn check_rule_names(flag: &str, names: &[String]) -> Result<()> {
    let known: Vec<&str> = check::rules::catalog().iter().map(|r| r.info().id).collect();
    for n in names {
        if !n.starts_with("x.") && !known.contains(&n.as_str()) {
            let hint = if known.contains(&n.to_uppercase().as_str()) {
                format!("; rule ids are upper case: {flag} {}", n.to_uppercase())
            } else {
                String::new()
            };
            bail!("{flag} {n}: no such rule (spec explain lists them){hint}");
        }
    }
    Ok(())
}

/// The files named on the command line, repository-relative; each must be a document file.
pub(crate) fn documents_named(s: &session::Session, files: &[String], stdin: Option<&str>) -> Result<Vec<String>> {
    let mut out = vec![];
    for f in files {
        if f == "-" {
            match stdin {
                Some(_) => continue,
                None => bail!(
                    "-: spec check reads standard input only for a named path; next: spec check --stdin-path <FILE>  # text on stdin checked as <FILE>"
                ),
            }
        }
        let abs = s.loc.absolute(std::path::Path::new(f));
        let rel = paths::rel_of(&s.corpus.cfg.spec_home, &abs).unwrap_or_else(|| s.loc.repo_path(f));
        // B2: a folder names the documents under it; one with none is refused, not checked empty.
        if std::path::Path::new(f).is_dir() {
            let under = |p: &str| rel.is_empty() || rel == "." || paths::is_below(p, rel.trim_end_matches('/'));
            let docs: Vec<String> = s.corpus.docs.iter().filter(|d| under(&d.path)).map(|d| d.path.clone()).collect();
            if docs.is_empty() {
                bail!("{f}: a folder with no documents under it; next: spec check  # every document");
            }
            out.extend(docs);
            continue;
        }
        let known = s.corpus.docs.iter().any(|d| d.path == rel || d.chapters.iter().any(|c| c == &rel));
        if !known && !std::path::Path::new(f).exists() {
            // Anything that is not a Markdown path is read the way `spec resolve` reads it: an id,
            // alias, slug or id prefix, with the candidates when it is ambiguous.
            if !f.ends_with(".md") {
                let idx = session::resolve(s, f)?;
                out.push(s.corpus.docs[idx].path.clone());
                continue;
            }
            if stdin == Some(f.as_str()) {
                let parent = std::path::Path::new(&rel).parent().map(paths::slash);
                let sibling =
                    s.corpus.docs.iter().find(|d| std::path::Path::new(&d.path).parent().map(paths::slash) == parent);
                match sibling {
                    Some(d) => {
                        let kind = d.id.to_string().split(':').next().unwrap_or("").to_string();
                        bail!(
                            "{f}: the name does not follow this directory's document naming (like {}), so it is not a document; next: spec new {kind} --title <TITLE> --dry-run  # prints a path to use",
                            d.path
                        )
                    }
                    None => bail!(
                        "{f}: not in a spec directory, so the buffer is not a document; next: spec doctor  # lists the spec directories"
                    ),
                }
            }
            let stem = std::path::Path::new(f).file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
            let stem = stem.split('.').next().unwrap_or(&stem).to_string();
            bail!("{f}: no such file; next: spec find {}", specfold_core::suggest::shell::quote(&stem));
        }
        if !known {
            bail!(
                "{f}: not a document (spec check <FILE> takes a document file; `spec doctor` lists files it skipped)"
            );
        }
        out.push(rel);
    }
    Ok(out)
}

/// `--path`: the files named, as repository paths; they need not exist (a file about to be
/// written is read from standard input).
pub(super) fn repo_paths(s: &session::Session, named: &[String]) -> Vec<String> {
    named
        .iter()
        .map(|f| {
            let abs = s.loc.absolute(std::path::Path::new(f));
            paths::rel_of(&s.corpus.cfg.spec_home, &abs).unwrap_or_else(|| s.loc.repo_path(f))
        })
        .collect()
}

/// B1's measures of the `--path` files, for the `budgets` field; empty when the budgets cannot
/// be read (B1 reports that as an error).
pub(super) fn budget_measures(s: &session::Session, named: &[String]) -> serde_json::Value {
    use specfold_core::check::limits::budgets;
    let Ok(list) = budgets::load(&s.corpus.cfg) else { return serde_json::json!([]) };
    let measured: Vec<budgets::Measure> = named
        .iter()
        .filter_map(|p| {
            let text = specfold_core::corpus::overlay::read(&s.corpus.cfg.spec_home.join(p)).ok()?;
            budgets::measure(&list, p, text.as_bytes())
        })
        .collect();
    serde_json::to_value(measured).unwrap_or_default()
}

/// `--stdin-path`: standard input stands in for that file's contents.
pub(super) fn overlay_stdin(loc: &session::Location, p: &str) -> Result<()> {
    let abs = loc.absolute(std::path::Path::new(p));
    let mut text = String::new();
    std::io::Read::read_to_string(&mut std::io::stdin(), &mut text)?;
    specfold_core::corpus::overlay::set(abs, text);
    Ok(())
}

/// C4: the base..head a `--changed`/`--staged` run compared, as JSON and a foot line holding
/// the resolved commits, so the same check can be rerun locally on the exact base.
pub(super) fn checked_range(root: &std::path::Path, a: &CheckArgs) -> Option<(serde_json::Value, String)> {
    use specfold_core::workspace::vcs;
    let (base, head) = match (&a.changed, a.staged) {
        (Some(b), _) => (b.clone(), "working tree"),
        (None, true) => ("HEAD".to_string(), "index"),
        (None, false) => return None,
    };
    let base_commit = vcs::resolve(root, &base);
    let head_commit = vcs::head(root);
    let short = |c: &Option<String>| c.as_deref().map(|c| c.chars().take(12).collect::<String>());
    let line = format!(
        "checked: {base} ({})..{head} (HEAD {}); rerun: spec check {}",
        short(&base_commit).unwrap_or_else(|| "unresolved".into()),
        short(&head_commit).unwrap_or_else(|| "none".into()),
        match (&a.changed, &base_commit) {
            (Some(_), Some(c)) => format!("--changed {}", &c[..c.len().min(12)]),
            (Some(b), None) => format!("--changed {b}"),
            _ => "--staged".to_string(),
        }
    );
    let json = serde_json::json!({
        "base": base, "base_commit": base_commit, "head": head, "head_commit": head_commit,
    });
    Some((json, line))
}

pub(super) fn range_of(a: &CheckArgs) -> Range {
    match (&a.changed, a.staged) {
        (Some(base), _) => Range::Changed(base.clone()),
        (None, true) => Range::Staged,
        (None, false) => Range::All,
    }
}
