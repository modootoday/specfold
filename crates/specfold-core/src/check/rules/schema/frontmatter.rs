//! S1: the frontmatter parses and holds the common keys, and every other key is declared.

use crate::check::context::{Ctx, key_line};
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::shell::quote;
use crate::suggest::distance;

/// Frontmatter keys every document may carry.
const COMMON_KEYS: &[&str] = &[
    "id",
    "title",
    "status",
    "created",
    "updated",
    "owners",
    "tags",
    "aliases",
    "rel",
    "files",
    "generated_by",
    "check_skip",
    "verified_at",
    "lang",
    "source_rev",
    "review_by",
    "pins",
];
const REQUIRED_KEYS: &[&str] = &["id", "title", "status", "created", "updated"];

pub struct Frontmatter;

impl Rule for Frontmatter {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "S1",
            title: "frontmatter parses and has the required keys",
            explain: "Every document starts with a YAML frontmatter block holding id, title, status, created and updated. Keys outside the common set and the type's own fields are reported so typos do not pass silently; a key that is meant to stay is declared in spec.toml as `[types.<type>] fields = [...]`, a list that replaces the type's built-in fields, so it keeps them, or for every type as `[check] extra_keys = [\"team\", \"review_*\"]` (a trailing * allows a prefix). `spec fix --rule S1` fills missing keys from the path and the first heading. A file that is not UTF-8 is refused; convert it from its encoding with `iconv -f <encoding> -t UTF-8 <file>`.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers(i) {
                continue;
            }
            if let Some(e) = &d.frontmatter.error
                && e.starts_with(crate::corpus::overlay::UTF8)
            {
                let line = crate::corpus::overlay::utf8_line(e).unwrap_or(1);
                out.push(Finding::error("S1", &d.path, line, format!("file is {e}; save it as UTF-8")));
                continue;
            }
            if let Some(e) = &d.frontmatter.error {
                out.push(Finding::error("S1", &d.path, 1, format!("frontmatter does not parse: {e}")));
                continue;
            }
            let Some(fm) = d.fm() else {
                out.push(
                    Finding::error("S1", &d.path, 1, "no frontmatter block")
                        .fix_cmd(format!("spec fix --rule S1 --write {}", quote(&d.path))),
                );
                continue;
            };
            let missing: Vec<&str> = REQUIRED_KEYS.iter().copied().filter(|k| !fm.contains_key(*k)).collect();
            if !missing.is_empty() {
                out.push(
                    Finding::error("S1", &d.path, 1, format!("missing {}", missing.join(", ")))
                        .fix_cmd(format!("spec fix --rule S1 --write {}", quote(&d.path))),
                );
            }
            // C200: a title of the wrong YAML type (a list, a map, a number, "") is no title.
            if let Some(t) = fm.get("title")
                && !t.as_str().is_some_and(|s| !s.trim().is_empty())
            {
                out.push(
                    Finding::error(
                        "S1",
                        &d.path,
                        key_line(ctx, &d.path, "title"),
                        format!("title is {t}, not a non-empty string"),
                    )
                    .fix_cmd(format!("spec set {} 'title=<text>' --write", d.id)),
                );
            }
            let ty = ctx.corpus.cfg.registry.get(&d.id.ty);
            for k in fm.keys() {
                let known = COMMON_KEYS.contains(&k.as_str())
                    || ctx.corpus.cfg.raw.check.extra_key(k)
                    || ty.is_some_and(|t| t.fields.iter().any(|f| f == k) || (t.sealed && k == "sealed"));
                if !known {
                    out.push(Finding::warn(
                        "S1",
                        &d.path,
                        key_line(ctx, &d.path, k),
                        unknown_key_message(k, &d.id.ty, ty.map(|t| t.fields.as_slice()).unwrap_or_default()),
                    ));
                }
            }
        }
    }
}

/// The S1 warning `spec set` would cause by writing `key` on a document of type `ty`, so set can
/// say it at once.
pub fn unknown_key(c: &crate::corpus::Corpus, ty: &str, key: &str) -> Option<String> {
    let k = key.split('.').next().unwrap_or(key);
    let t = c.cfg.registry.get(ty);
    let known = COMMON_KEYS.contains(&k)
        || c.cfg.raw.check.extra_key(k)
        || t.is_some_and(|t| t.fields.iter().any(|f| f == k) || (t.sealed && k == "sealed"));
    (!known).then(|| unknown_key_message(k, ty, t.map(|t| t.fields.as_slice()).unwrap_or_default()))
}

/// Names a close known key when the unknown one looks like a typo, and otherwise shows the
/// spec.toml line that declares it. `fields` replaces the type's list, so the line keeps it.
fn unknown_key_message(k: &str, ty: &str, fields: &[String]) -> String {
    let known = COMMON_KEYS.iter().map(|s| s.to_string()).chain(fields.iter().cloned());
    if let Some(near) = known.filter(|c| c != k).find(|c| distance(c, k) <= 2.min(k.len() / 3)) {
        return format!("unknown key '{k}' for {ty}; did you mean '{near}'?");
    }
    let list: Vec<String> = fields.iter().map(String::as_str).chain([k]).map(|f| format!("\"{f}\"")).collect();
    format!(
        "unknown key '{k}' for {ty}; to keep it, declare it in spec.toml: [types.{ty}] fields = [{}], or for every type: [check] extra_keys = [\"{k}\"]",
        list.join(", ")
    )
}

#[cfg(test)]
mod unknown_key_tests {
    #[test]
    fn suggests_a_near_key_or_the_declaration() {
        let f = vec!["verify".to_string()];
        assert!(super::unknown_key_message("staus", "adr", &f).ends_with("did you mean 'status'?"));
        assert_eq!(
            super::unknown_key_message("domain", "adr", &f),
            "unknown key 'domain' for adr; to keep it, declare it in spec.toml: [types.adr] fields = [\"verify\", \"domain\"], or for every type: [check] extra_keys = [\"domain\"]"
        );
    }
}
