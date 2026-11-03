//! `spec explain`: the rule's own explanation is the documentation of record.

use crate::cli::args::{global::Global, setup::ExplainArgs};
use crate::output::Output;
use crate::session;
use anyhow::{Result, bail};
use serde_json::json;
use specfold_core::check::rules::catalog;
use specfold_core::plugins;

const CONFIG_HELP: &[(&str, &str)] = &[
    (
        "[spec] schema, dir, submodule_remotes",
        "Format version (1), the name of each owner's document directory (.spec), and whether submodules that carry their own spec.toml are read as remotes named after their directory (off unless set).",
    ),
    (
        "[owners] root, kinds, manifests, retired, pending, nested",
        "The root owner id; owner kinds as pattern + id template ({1}, {2} …) + manifest requirement; the manifest list; directories whose code is gone; spec directories not yet imported; and nested: independent projects mounted here (a submodule with its own spec.toml, documents and CI). A nested project is never an owner: this repository does not read its documents (so its ids never clash with an owner's), governs into it are another project's (P1, L5), no walk from the root enters it (V1, V3, L6, N1, Z1, the census and [[assets]]), and one with a spec.toml is the read-only remote named after its last path segment. A submodule an owner kind claims is that owner's code; any other submodule is another project for placement.",
    ),
    (
        "[public] paths, companion, detect, submodules_manifest",
        "Published paths that may hold no spec document, the private owner each one's documents go to, detectors that find more (pages, site-generators, npm-files, cargo-include), and an offline submodule visibility manifest checked against .gitmodules, written by spec sync visibility.",
    ),
    ("[local] path", "The git-ignored directory for drafts and private notes."),
    (
        "[external.<name>] path, mode",
        "Directories outside the repository (an agent's memory) whose citations are checked with --external.",
    ),
    (
        "[files] inline_max_bytes, scan_max_bytes, store",
        "Size limit for attachments in the repository; the size up to which repository text attachments are read by the secrets check (U2, 1 MiB by default); the store for larger ones (dir or command).",
    ),
    (
        "[types.<name>]",
        "Override or disable a built-in type, or add one: key, form, statuses, initial, sealed, fields, template, transitions (from → [to], rule T2), require_when (status → required keys, rule S6), sections (headings every document needs, rule S7), and for rule S8: fields as a table of value kinds (people, url, date, link), placeholders (default TBD, Pending, TODO, ?) and distinct groups.",
    ),
    ("[forbid] paths, patterns", "Retired layouts and identifiers that must not come back (rule Z1)."),
    ("[aliases] file", "The frozen alias table written by spec import."),
    (
        "[[guards]]",
        "Named checks over files and commit messages (rule U1, spec guard): paths, public (every published path but nested projects), nested (every [owners] nested project, so its list is kept once), exclude, range, deny_terms(_file, _env), deny_regex and deny_path_regex (a pattern, lookaround allowed, or a table: re, label, not_in_url, not_in_regex_literal, code_exempt, exempt, must_hit, must_pass), deny_files, max_file_bytes, secrets, commit_messages, level, allow, fixtures, decompress, read_binary, fail_if_empty, baseline.",
    ),
    (
        "[[assets]] kind, path, exclude, copy_of",
        "Agent assets read beside the documents: kind skill, rule, agent or instructions; a directory or glob (rules A1, A2; spec assets). copy_of names a source directory this root copies; only spec sync assets writes such a root, previewing until --write and deleting only with --prune.",
    ),
    ("[[plugins]]", "External programs speaking specfold.plugin.v1: rules, importers, exporters, audits, events."),
    (
        "[audit]",
        "Thresholds for stale, stalled and draft documents, open-item markers, and the types whose anchors must be cited (coverage).",
    ),
    (
        "[check] baseline, strict, severity, extra_keys, count_paths, d1, remote_status",
        "A committed file of known findings so only new ones fail (spec check --write-baseline), paths that never get a baseline, per-rule levels (error, warn, off), and frontmatter keys every type may carry (rule S1; `review_*` allows a prefix), and Markdown files outside documents whose spec:count markers are checked (rule N1; `README.md`, `docs/**/*.md`). d1 = \"specific\" asks only the most specific governing documents (D1); remote_status = true warns when a relation names a superseded or archived document in a remote (L3).",
    ),
    (
        "[[check.override]] paths, rule, level",
        "A rule's level (off, warn, error) for findings in files under the given globs, applied after [check] severity; the last matching entry wins.",
    ),
    (
        "[verify] timeout_ms, jobs, serial, full_run_on, shell",
        "How spec verify runs commands: the default deadline, parallel workers, regexes for heavy commands run one at a time, paths whose change runs every check (an edit of spec.toml that touches only [remotes] or [check] baseline does not; a full run says why on stderr), and the shell (sh unless set, bash for bash syntax).",
    ),
    (
        "[write] max_files",
        "mv, fix, sync and import refuse --write without --yes when more files than this would change (default 50; 0 means no bound). The preview says how many.",
    ),
    (
        "[mcp] redact",
        "Whether spec mcp replaces built-in secret matches (the guard secrets list) with [redacted: <pattern>] in tool results; on unless set to false.",
    ),
    (
        "[cites] paths, exclude, history",
        "Files outside documents (code, READMEs, agent instructions) whose citations rule L6 checks, and regexes for history lines it leaves alone.",
    ),
    (
        "[remotes.<name>] path",
        "Another repository already on disk (a sibling checkout, a submodule, a CI checkout) whose documents are named <name>:<id> in references and relations. Read-only: L1 and L3 check those targets when the directory exists and skip them when it does not; spec show and spec resolve read them. A name equal to a type name is not a remote. Id prefixes and a private root for a repository without spec.toml can also come from the user-level ~/.config/specfold/roots.toml ($SPECFOLD_ROOTS).",
    ),
    (
        "[translations] layout, languages, source_lang, strict",
        "Translations paired by place (rule R2): layout suffix (`<slug>-<lang>` beside the source) or dir (the same slug under a `<lang>` owner), none by default; the languages the layout and spec census --translations use, the sources' language, and strict to make R2 errors. rel.translation_of and lang in frontmatter win over the layout.",
    ),
];

pub fn run(g: &Global, a: &ExplainArgs) -> Result<Output> {
    let mut o = Output::new("explain");
    match a.topic.as_deref() {
        Some("config") => {
            for (k, v) in CONFIG_HELP {
                o = o.item(json!({"key": k, "explain": v}), format!("{k}\n  {v}"));
            }
            return Ok(o.next("spec init"));
        }
        Some(t) if t.eq_ignore_ascii_case("status") || t.eq_ignore_ascii_case("lifecycle") => {
            return status(g, o);
        }
        Some(t) if t.starts_with("x.") => {
            let cfg = session::config(g)?;
            for p in plugins::load(&cfg) {
                let Ok(d) = p.description else { continue };
                for r in d.provides.rules {
                    if format!("x.{}.{}", p.def.name, r.id) == t {
                        return Ok(o.item(
                            json!({"id": t, "title": r.title, "explain": r.explain}),
                            format!("{t}  {}\n\n{}", r.title, r.explain),
                        ));
                    }
                }
            }
            bail!("no plugin rule '{t}'");
        }
        _ => {}
    }
    let rules = catalog();
    let picked: Vec<_> =
        rules.iter().filter(|r| a.topic.as_deref().is_none_or(|t| r.info().id.eq_ignore_ascii_case(t))).collect();
    if picked.is_empty() {
        bail!("no rule '{}' (spec explain lists them all)", a.topic.clone().unwrap_or_default());
    }
    let first = picked[0].info().id;
    for r in picked {
        let i = r.info();
        let text = if a.topic.is_some() {
            format!("{}  {}\n\n{}\n\nruns on commit: {}", i.id, i.title, i.explain, i.on_commit)
        } else {
            format!("{:<4} {}", i.id, i.title)
        };
        o = o.item(json!({"id": i.id, "title": i.title, "explain": i.explain, "on_commit": i.on_commit}), text);
    }
    if a.topic.is_none() {
        o = o.foot(format!("other topics: {}", TOPICS.join(", ")));
    }
    // D1 never reports in a full run: point at the run that shows it.
    let staged = if first == "D1" { " --staged" } else { "" };
    Ok(o.next(format!("spec check{staged} --rule {first}")))
}

/// Topics beside rule ids, named in the listing.
const TOPICS: &[&str] = &["config", "status (or lifecycle)", "x.<plugin>.<rule>"];

const LIFECYCLE_HELP: &str = "A status moves forward only (rule T2): draft -> active -> superseded or archived, draft -> rejected, and draft -> superseded once a newer document names it in rel.supersedes. Supersession is written on the newer document (rule T1): `spec new <type> --from <old>` writes both sides; for a newer document that already exists, `spec set <newer> rel.supersedes+=<old> --write`, then `spec set <old> status=superseded --write`. A type with `transitions` in spec.toml moves only as they say.";

/// B1: every type's statuses and where each may move, from the configuration in force.
fn status(g: &Global, mut o: Output) -> Result<Output> {
    let cfg = session::config(g)?;
    o = o.head(LIFECYCLE_HELP).field("explain", LIFECYCLE_HELP);
    for name in cfg.registry.names() {
        let Some(t) = cfg.registry.get(name) else { continue };
        let moves = specfold_core::model::doctype::status_lines(t);
        o = o.item(
            json!({"type": t.name, "initial": t.initial, "statuses": t.statuses, "moves": moves}),
            format!("{:<10} {}", t.name, moves.join("; ")),
        );
    }
    Ok(o.next("spec explain T2"))
}
