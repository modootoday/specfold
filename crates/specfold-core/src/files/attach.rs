//! Attaching files to a document: decide where each file is kept, copy or upload it, and record
//! it in the document's `files` list.

use super::{hash, ignore_file, store};
use crate::corpus::Corpus;
use crate::markdown::frontmatter::edit;
use crate::model::Doc;
use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    /// Decide by size: small files in the repository, large ones in the store.
    Auto,
    Repo,
    Store,
}

#[derive(Debug, Clone, Serialize)]
pub struct Planned {
    pub source: String,
    /// Path inside the bundle.
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
    pub store: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Plan {
    pub doc: String,
    /// Set when a single-file document becomes a bundle first.
    pub to_bundle: Option<(String, String)>,
    pub files: Vec<Planned>,
}

fn bundle_main(doc: &Doc) -> (String, String) {
    let file = doc.path.rsplit_once('/').map(|x| x.1).unwrap_or(&doc.path);
    let dir = doc.base_dir();
    let stem = file.trim_end_matches(".md").trim_end_matches(&format!(".{}", doc.id.ty));
    let new_dir = format!("{dir}/{stem}");
    (new_dir.clone(), format!("{new_dir}/index.{}.md", doc.id.ty))
}

pub fn plan(c: &Corpus, doc: &Doc, sources: &[PathBuf], into: &str, place: Where) -> Result<Plan> {
    let limit = c.cfg.raw.files.inline_max_bytes;
    let has_store = c.cfg.raw.files.store.is_some();
    let to_bundle = match &doc.dir {
        Some(_) => None,
        None => Some((doc.path.clone(), bundle_main(doc).1)),
    };
    // C142: --into stays inside the bundle.
    if into.starts_with('/')
        || into.starts_with('\\')
        || into.contains(':')
        || into.split(['/', '\\']).any(|p| p == "..")
    {
        bail!("--into '{into}' must be a relative folder inside the bundle (no '..', no absolute path)");
    }
    let mut files = vec![];
    for src in sources {
        let (sha256, bytes) = hash::file(src).with_context(|| format!("cannot read {}", src.display()))?;
        let name = src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let rel = if into.is_empty() { name } else { format!("{}/{name}", into.trim_matches('/')) };
        let store = match place {
            Where::Repo if bytes > limit => {
                bail!("{} is {bytes} bytes, over the {limit}-byte limit for the repository", src.display())
            }
            Where::Repo => "repo",
            Where::Store if !has_store => bail!("no [files.store] is configured"),
            Where::Store => "store",
            Where::Auto if bytes > limit && !has_store => {
                bail!("{} is {bytes} bytes; configure [files.store] or shrink it under {limit}", src.display())
            }
            Where::Auto if bytes > limit => "store",
            Where::Auto => "repo",
        };
        files.push(Planned {
            source: src.to_string_lossy().into_owned(),
            path: rel,
            sha256,
            bytes,
            store: store.into(),
        });
    }
    Ok(Plan { doc: doc.id.to_string(), to_bundle, files })
}

pub fn apply(c: &Corpus, doc: &Doc, plan: &Plan) -> Result<()> {
    let root = &c.cfg.root;
    let main = match &plan.to_bundle {
        Some((from, to)) => {
            std::fs::create_dir_all(root.join(to).parent().unwrap_or(root))?;
            std::fs::rename(root.join(from), root.join(to))?;
            to.clone()
        }
        None => doc.path.clone(),
    };
    let bundle = Path::new(&main).parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    let mut ignored = vec![];
    for f in &plan.files {
        let dst = root.join(&bundle).join(&f.path);
        if let Some(p) = dst.parent() {
            std::fs::create_dir_all(p)?;
        }
        // Re-attaching a file already in the bundle must not copy it onto itself: that truncates it.
        let same =
            std::fs::canonicalize(&f.source).ok().zip(std::fs::canonicalize(&dst).ok()).is_some_and(|(a, b)| a == b);
        if !same {
            std::fs::copy(&f.source, &dst).with_context(|| format!("cannot copy {} to {}", f.source, dst.display()))?;
        }
        if f.store == "store" {
            let s = store::open(&c.cfg)?.context("no [files.store] is configured")?;
            s.put(&dst, &f.sha256)?;
            ignored.push(f.path.clone());
        }
    }
    if !ignored.is_empty() {
        ignore_file::add(&root.join(&bundle), &ignored)?;
    }
    let text = std::fs::read_to_string(root.join(&main))?;
    let new = edit::rewrite(&text, |m| {
        let mut list: Vec<Value> = m.get("files").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        for f in &plan.files {
            list.retain(|v| v.get("path").and_then(Value::as_str) != Some(f.path.as_str()));
            list.push(json!({"path": f.path, "sha256": f.sha256, "bytes": f.bytes, "store": f.store}));
        }
        m.insert("files".into(), Value::Array(list));
        m.insert("updated".into(), Value::String(crate::model::stamp::now_value()));
    })?;
    std::fs::write(root.join(&main), new)?;
    Ok(())
}
