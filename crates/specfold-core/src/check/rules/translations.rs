//! R1: a translation records the source body it was made from, and says when the source moved on.
//! R2: a translation is where the `[translations]` layout expects it.

use super::support::key_line;
use crate::check::context::Ctx;
use crate::check::finding::{Finding, Severity};
use crate::check::rule::{Rule, RuleInfo};
use crate::translation::{self, KEY, RELATION};

pub struct TranslationsCurrent;

impl Rule for TranslationsCurrent {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "R1",
            title: "translations are current with their source",
            explain: "A translated document names its source with rel.translation_of and records the source it was made from as source_rev: sha256:<12 hex>, a hash of the source body (main file, then the bundle's chapters in path order) with frontmatter removed and whitespace collapsed (no git history needed, and reformatting does not count). When git still has a commit with the recorded revision, the warning names the sections that changed since, with the number of paragraphs changed, new and removed sections (chapters are prefixed with their file name). When the source body changes, the translation is reported as behind until it is re-translated and source_rev is recorded again with `spec set <id> --source-rev`. A translation without source_rev gets the same command. A source in a remote (`rel.translation_of: [<remote>:<id>]`) is compared the same way when the remote directory is present, without section names. A source that does not resolve is reported by L3. R1 also checks the values: lang is a language tag (BCP 47: `ko`, `pt-BR`), source_rev is `sha256:` and 12 hex digits, one source has one translation per language, and a translation names an original, not itself or another translation (those two are errors). Example frontmatter: `lang: ko`, `rel: {translation_of: [guide:shop/setup]}`, `source_rev: sha256:3f2a9c0b1d4e`; `spec new --translation ko --from guide:shop/setup` writes all three. Reported as warnings; `spec census --translations` lists every translation and the sources each language lacks.",
            on_commit: false,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        let c = ctx.corpus;
        // Translations per (source, language), for two translations into one language.
        let mut per: std::collections::BTreeMap<(usize, String), Vec<usize>> = Default::default();
        for i in 0..c.docs.len() {
            if let (true, Some(s), Some(l)) =
                (translation::is_translation(c, i), translation::source_of(c, ctx.graph, i), translation::lang_of(c, i))
            {
                per.entry((s, l)).or_default().push(i);
            }
        }
        for (i, d) in c.docs.iter().enumerate() {
            if ctx.scope.covers(i)
                && let Some(p) = d.str_field("lang").and_then(|l| translation::lang_problem(&l))
            {
                out.push(Finding::warn("R1", &d.path, key_line(ctx, &d.path, "lang"), p));
            }
            if !translation::is_translation(c, i) {
                continue;
            }
            let targets = d.rel().get(RELATION).cloned().unwrap_or_default();
            let Some(src) = translation::source_of(c, ctx.graph, i) else {
                // A source in a remote repository: compare its body with source_rev.
                if ctx.scope.covers(i)
                    && let Some((rid, now)) =
                        translation::source_ref(c, i).and_then(|r| crate::remotes::rev(&c.cfg, &r))
                {
                    let fix = format!("spec set {} --source-rev", crate::shell::word(&d.id));
                    let line = key_line(ctx, &d.path, KEY);
                    match d.str_field(KEY) {
                        None => out.push(
                            Finding::warn("R1", &d.path, 1, format!("translation of {rid} records no source_rev"))
                                .fix_cmd(fix),
                        ),
                        Some(r) if translation::rev_problem(&r).is_some() => out.push(
                            Finding::warn("R1", &d.path, line, translation::rev_problem(&r).unwrap_or_default())
                                .fix_cmd(fix),
                        ),
                        Some(r) if r != now => out.push(
                            Finding::warn(
                                "R1",
                                &d.path,
                                line,
                                format!("{rid} changed since source_rev {r}: update the translation"),
                            )
                            .fix_cmd(fix),
                        ),
                        Some(_) => {}
                    }
                }
                continue;
            };
            if !(ctx.scope.covers(i) || ctx.scope.covers(src)) {
                continue;
            }
            let rel_line = key_line(ctx, &d.path, "rel");
            if src == i {
                out.push(Finding::error("R1", &d.path, rel_line, "rel.translation_of names the document itself"));
                continue;
            }
            if translation::is_translation(c, src) {
                let orig = translation::source_of(c, ctx.graph, src)
                    .filter(|&o| o != src)
                    .map(|o| format!("; name its source {}", c.docs[o].id))
                    .unwrap_or_default();
                out.push(Finding::error(
                    "R1",
                    &d.path,
                    rel_line,
                    format!("source {} is itself a translation{orig}", c.docs[src].id),
                ));
                continue;
            }
            if let Some(l) = translation::lang_of(c, i)
                && let Some(same) = per.get(&(src, l.clone())).filter(|v| v.len() > 1)
            {
                let others: Vec<String> = same.iter().filter(|&&j| j != i).map(|&j| c.docs[j].id.to_string()).collect();
                out.push(Finding::warn(
                    "R1",
                    &d.path,
                    rel_line,
                    format!("{} has another '{l}' translation: {}", c.docs[src].id, others.join(", ")),
                ));
            }
            let line = key_line(ctx, &d.path, KEY);
            let fix = format!("spec set {} --source-rev", crate::shell::word(&d.id));
            if targets.len() > 1 {
                out.push(Finding::warn(
                    "R1",
                    &d.path,
                    key_line(ctx, &d.path, "rel"),
                    format!("rel.translation_of names {} documents; a translation has one source", targets.len()),
                ));
            }
            let source = &c.docs[src];
            let now_parts = translation::parts_of(c, src, |p| ctx.current(p));
            let now = now_parts.as_ref().map(|p| translation::rev_of_parts(p));
            match d.str_field(KEY) {
                None => out.push(
                    Finding::warn("R1", &d.path, 1, format!("translation of {} records no source_rev", source.id))
                        .fix_cmd(fix),
                ),
                Some(r) if translation::rev_problem(&r).is_some() => out.push(
                    Finding::warn("R1", &d.path, line, translation::rev_problem(&r).unwrap_or_default()).fix_cmd(fix),
                ),
                Some(r) if now.as_deref().is_some_and(|n| n != r) => {
                    // Name the sections when git still has the recorded revision.
                    let what = now_parts
                        .as_deref()
                        .and_then(|p| translation::changes_since(c, src, &r, p))
                        .filter(|ch| !ch.is_empty())
                        .map(|ch| format!(" in {}", translation::describe(&ch)))
                        .unwrap_or_default();
                    out.push(
                        Finding::warn(
                            "R1",
                            &d.path,
                            line,
                            format!("{} changed since source_rev {r}{what}: update the translation", source.id),
                        )
                        .fix_cmd(fix),
                    )
                }
                Some(_) => {}
            }
        }
    }
}

pub struct TranslationsPlaced;

impl Rule for TranslationsPlaced {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "R2",
            title: "translations are where the layout expects them",
            explain: "With `[translations] layout = \"suffix\"` a translation is the document `<slug>-<lang>` beside its source (same type and spec directory); with `layout = \"dir\"` it has the source's slug in a spec directory under a `<lang>` directory (`docs/ko/.spec/guide/setup.guide.md` for `docs/.spec/guide/setup.guide.md`). `<lang>` is one of `languages`. Such a pair needs no frontmatter; rel.translation_of and lang, when written, win. R2 reports a document in a translation position whose source does not exist or is not unique, rel.translation_of that names another document than the layout pairs it with, and lang that differs from the language in its name. Warnings, errors with `strict = true`. `spec fix --translations --write` writes rel.translation_of and lang into every inferred pair.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        let c = ctx.corpus;
        let strict = c.cfg.raw.translations.strict;
        let mut push = |f: Finding| {
            out.push(if strict { Finding { severity: Severity::Error, ..f } } else { f });
        };
        for (i, d) in c.docs.iter().enumerate() {
            if !ctx.scope.covers(i) {
                continue;
            }
            let Some(inf) = translation::inferred(c, i) else { continue };
            let explicit = translation::source_ref(c, i);
            if let Some(l) = d.str_field("lang").filter(|l| l != &inf.lang) {
                push(Finding::warn(
                    "R2",
                    &d.path,
                    key_line(ctx, &d.path, "lang"),
                    format!("lang is '{l}' but its place makes it the '{}' translation", inf.lang),
                ));
            }
            match (inf.source, &explicit) {
                (Some(s), Some(_)) => {
                    let named = translation::source_of(c, ctx.graph, i);
                    if named.is_some_and(|n| n != s) {
                        push(Finding::warn(
                            "R2",
                            &d.path,
                            key_line(ctx, &d.path, "rel"),
                            format!(
                                "rel.translation_of names {} but the layout pairs it with {}",
                                c.docs[named.unwrap_or(s)].id,
                                c.docs[s].id
                            ),
                        ));
                    }
                }
                (None, None) if inf.candidates.is_empty() => push(Finding::warn(
                    "R2",
                    &d.path,
                    1,
                    format!(
                        "placed as the '{}' translation but no source is in the matching place; add the source or set rel.translation_of",
                        inf.lang
                    ),
                )),
                (None, None) => push(Finding::warn(
                    "R2",
                    &d.path,
                    1,
                    format!(
                        "placed as the '{}' translation of {} documents ({}); set rel.translation_of",
                        inf.lang,
                        inf.candidates.len(),
                        inf.candidates.iter().map(|&j| c.docs[j].id.to_string()).collect::<Vec<_>>().join(", ")
                    ),
                )),
                _ => {}
            }
        }
    }
}
