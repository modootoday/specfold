//! P2, P3, E1, E2: every non-Markdown file belongs to a document's `files` list, small files stay
//! in the repository, and recorded hashes match.

use super::support::quote;
use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::files::hash_cache::HashCache;
use crate::files::{ignore_file, store};

pub struct Registered;
pub struct SizeLimit;
pub struct HashesMatch;
pub struct StoredExist;

impl Rule for Registered {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "P2",
            title: "every attachment is registered",
            explain: "A file inside a document bundle must appear in that document's `files` list, and every listed repository file must exist. `spec attach <id> <file>` copies and registers in one step.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers(i) {
                continue;
            }
            let base = d.base_dir();
            let entries = d.files();
            for f in &d.bundle_files {
                let rel = f.strip_prefix(&format!("{base}/")).unwrap_or(f);
                if rel == ignore_file::NAME || entries.iter().any(|e| e.path == rel) {
                    continue;
                }
                out.push(Finding::error("P2", f, 1, format!("not listed in {}'s files", d.id)).fix_cmd(format!(
                    "spec attach {} {} --write",
                    d.id,
                    quote(f)
                )));
            }
            for e in entries.iter().filter(|e| e.store == "repo") {
                if !ctx.corpus.cfg.root.join(&base).join(&e.path).is_file() {
                    out.push(Finding::error(
                        "P2",
                        &d.path,
                        1,
                        format!("files lists {} but it is not in the bundle", e.path),
                    ));
                }
            }
        }
    }
}

impl Rule for SizeLimit {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "P3",
            title: "repository attachments stay small",
            explain: "Files kept in the repository are limited to [files] inline_max_bytes (256 KiB by default). Larger files, and raw machine output of any size, go to the store; `spec attach` decides this automatically.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        let limit = ctx.corpus.cfg.raw.files.inline_max_bytes;
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers(i) {
                continue;
            }
            let stored: Vec<String> = d.files().into_iter().filter(|e| e.store == "store").map(|e| e.path).collect();
            for f in &d.bundle_files {
                let Ok(meta) = std::fs::metadata(ctx.corpus.cfg.root.join(f)) else { continue };
                if meta.len() > limit && !stored.iter().any(|p| f.ends_with(p.as_str())) {
                    out.push(
                        Finding::error(
                            "P3",
                            f,
                            1,
                            format!("{} bytes is over the {limit}-byte repository limit", meta.len()),
                        )
                        .fix_cmd(format!("spec attach {} {} --store --write", d.id, quote(f))),
                    );
                }
            }
        }
    }
}

impl Rule for HashesMatch {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "E1",
            title: "recorded attachments carry a matching hash",
            explain: "Each `files` entry has path, sha256, bytes and store. For repository files the hash must match the content, so an edited board or dataset cannot pass as the one a decision was made on. Re-run `spec attach` after changing a file.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        let mut hashes = HashCache::open(&ctx.corpus.cfg.root);
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers(i) {
                continue;
            }
            let base = ctx.corpus.cfg.root.join(d.base_dir());
            for e in d.files() {
                if e.sha256.len() != 64 || !["repo", "store"].contains(&e.store.as_str()) {
                    out.push(Finding::error(
                        "E1",
                        &d.path,
                        1,
                        format!("files entry {} needs sha256 and store (repo or store)", e.path),
                    ));
                    continue;
                }
                if e.store != "repo" {
                    continue;
                }
                if let Ok((got, bytes)) = hashes.file(&base.join(&e.path))
                    && (got != e.sha256 || bytes != e.bytes)
                {
                    // A file that now holds a secret must not be attached again as it is: point at
                    // U2, which reports the same file, instead of re-recording its hash.
                    let full = base.join(&e.path);
                    let leaky = std::fs::metadata(&full)
                        .is_ok_and(|m| m.len() <= ctx.corpus.cfg.raw.files.scan_max_bytes)
                        && std::fs::read(&full)
                            .is_ok_and(|b| !crate::guards::secrets::find(&String::from_utf8_lossy(&b)).is_empty());
                    let f = if leaky {
                        Finding::error(
                            "E1",
                            &d.path,
                            1,
                            format!(
                                "{} changed since it was attached and now holds a secret (U2): remove the value before attaching it again",
                                e.path
                            ),
                        )
                        .fix_cmd("spec explain U2")
                    } else {
                        Finding::error("E1", &d.path, 1, format!("{} changed since it was attached", e.path)).fix_cmd(
                            format!("spec attach {} {} --write", d.id, quote(&format!("{}/{}", d.base_dir(), e.path))),
                        )
                    };
                    out.push(f);
                }
            }
        }
        hashes.save();
    }
}

impl Rule for StoredExist {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "E2",
            title: "stored attachments exist in the store",
            explain: "With --online, each attachment kept in the store is looked up there. Stores that cannot answer without side effects are skipped.",
            on_commit: false,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        if !ctx.opts.online {
            return;
        }
        let Ok(Some(s)) = store::open(&ctx.corpus.cfg) else { return };
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers(i) {
                continue;
            }
            for e in d.files().into_iter().filter(|e| e.store == "store") {
                if let Ok(Some(false)) = s.exists(&e.sha256) {
                    let short = e.sha256.get(..12).unwrap_or(&e.sha256);
                    out.push(Finding::warn("E2", &d.path, 1, format!("{} ({short}) is not in the store", e.path)));
                }
            }
        }
    }
}
