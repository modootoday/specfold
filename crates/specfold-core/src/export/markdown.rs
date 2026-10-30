//! The built-in `markdown` exporter: a plain Markdown tree laid out for one target (Obsidian,
//! MkDocs, Docusaurus or a GitHub wiki). `[[references]]` become that target's links; a
//! reference that leads nowhere in the tree (unresolved, in a remote, or to a document left out)
//! is written as plain text and reported. The output directory must be empty or a previous
//! export, recognised by its manifest, whose files alone are replaced.

use super::links::{self, Link};
use crate::corpus::Corpus;
use crate::graph::Graph;
use crate::markdown::{body, frontmatter};
use crate::model::{Tier, stamp::slugify};
use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

pub const MANIFEST: &str = ".specfold-export.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Target {
    Obsidian,
    Mkdocs,
    Docusaurus,
    GithubWiki,
}

impl Target {
    pub const ALL: [&str; 4] = ["obsidian", "mkdocs", "docusaurus", "github-wiki"];

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "obsidian" => Target::Obsidian,
            "mkdocs" => Target::Mkdocs,
            "docusaurus" => Target::Docusaurus,
            "github-wiki" => Target::GithubWiki,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Target::Obsidian => "obsidian",
            Target::Mkdocs => "mkdocs",
            Target::Docusaurus => "docusaurus",
            Target::GithubWiki => "github-wiki",
        }
    }

    fn flat(self) -> bool {
        self == Target::GithubWiki
    }

    /// Whether headings keep their `{#anchor}` (MkDocs with attr_list, Docusaurus).
    fn keeps_anchors(self) -> bool {
        matches!(self, Target::Mkdocs | Target::Docusaurus)
    }
}

#[derive(Debug, Clone)]
pub struct Options {
    pub target: Target,
    /// Include documents in the git-ignored local directory.
    pub local: bool,
    /// Start each page with a note naming its source file.
    pub banner: bool,
}

/// A reference written as plain text because it leads nowhere in the tree.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Unresolved {
    pub path: String,
    pub raw: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default)]
pub struct Tree {
    /// Relative path and content, in path order.
    pub files: BTreeMap<String, String>,
    pub unresolved: Vec<Unresolved>,
    pub docs: usize,
}

struct Page {
    idx: usize,
    /// Path in the tree, e.g. `shop/adr/cart-totals.md`.
    file: String,
    title: String,
    /// Body with chapters appended, and its resolved references.
    body: String,
    links: Vec<Link>,
    /// Explicit anchor -> heading text.
    headings: HashMap<String, String>,
}

/// Characters no target or file system accepts in a file name.
fn safe_name(s: &str) -> String {
    let out: String =
        s.chars().map(|c| if c.is_control() || "\\/:*?\"<>|#^[]".contains(c) { '-' } else { c }).collect();
    let out = out.trim_matches(|c: char| c == '-' || c == '.' || c.is_whitespace()).to_string();
    if out.is_empty() { "untitled".into() } else { out }
}

fn base_name(c: &Corpus, idx: usize) -> String {
    let d = &c.docs[idx];
    if !d.slug.is_empty() {
        return safe_name(&d.slug);
    }
    let t = d.title.clone().unwrap_or_default();
    let s = slugify(&t);
    if !s.is_empty() {
        return s;
    }
    if !t.trim().is_empty() {
        return safe_name(&t);
    }
    safe_name(&d.id.key)
}

/// The text of every file of a document, main body first, frontmatter removed.
fn full_body(c: &Corpus, idx: usize) -> String {
    let d = &c.docs[idx];
    let read = |p: &str| crate::corpus::overlay::read(&c.cfg.root.join(p)).unwrap_or_default();
    let mut out = frontmatter::split(&read(&d.path)).body.trim_start_matches(['\n', '\r']).to_string();
    let mut chapters = d.chapters.clone();
    chapters.sort();
    for ch in chapters {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
        out.push_str(frontmatter::split(&read(&ch)).body);
    }
    out
}

/// Builds the tree in memory; nothing is written.
pub fn build(c: &Corpus, g: &Graph, o: &Options) -> Tree {
    let mut picked: Vec<usize> = (0..c.docs.len()).filter(|&i| o.local || c.docs[i].tier == Tier::Committed).collect();
    picked.sort_by_key(|&i| c.docs[i].id.to_string());
    let mut taken: HashSet<String> = HashSet::new();
    for reserved in ["home.md", "_sidebar.md", "index.md"] {
        taken.insert(reserved.into());
    }
    let mut pages: Vec<Page> = Vec::new();
    let mut by_idx: HashMap<usize, usize> = HashMap::new();
    for &idx in &picked {
        let d = &c.docs[idx];
        let dir = if o.target.flat() { String::new() } else { format!("{}/{}/", safe_name(&d.id.owner), d.id.ty) };
        let base = base_name(c, idx);
        let mut file = format!("{dir}{base}.md");
        let mut n = 2;
        while !taken.insert(file.to_lowercase()) {
            file = format!("{dir}{base}-{n}.md");
            n += 1;
        }
        let text = full_body(c, idx);
        let scanned = body::scan(&text, 1, &body::path_cite_re(".spec"));
        let headings = scanned.anchors.into_iter().map(|a| (a.name, a.heading)).collect();
        let links = links::of_body(c, g, &text);
        by_idx.insert(idx, pages.len());
        pages.push(Page {
            idx,
            file,
            title: d.title.clone().unwrap_or_else(|| d.id.to_string()),
            body: text,
            links,
            headings,
        });
    }
    let mut tree = Tree { docs: pages.len(), ..Default::default() };
    for p in &pages {
        let mut out = String::new();
        if !o.target.flat() {
            out.push_str(&front(c, p, o.target, &pages, &by_idx));
        }
        let d = &c.docs[p.idx];
        if o.banner {
            out.push_str(&format!("> Exported by specfold from `{}`. Edit the source, not this file.\n\n", d.path));
        }
        if let Some(s) = d.status().filter(|s| s == "superseded" || s == "rejected") {
            out.push_str(&format!("> **Status: {s}**\n\n"));
        }
        out.push_str(&rewrite(p, o.target, &pages, &by_idx, &mut tree.unresolved, &d.path));
        tree.files.insert(p.file.clone(), out);
    }
    index_files(&mut tree, c, o.target, &pages);
    tree
}

fn yaml_str(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}

fn yaml_list(v: &[String]) -> String {
    format!("[{}]", v.iter().map(|s| yaml_str(s)).collect::<Vec<_>>().join(", "))
}

fn strings(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(a)) => a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        _ => vec![],
    }
}

fn front(c: &Corpus, p: &Page, t: Target, pages: &[Page], by_idx: &HashMap<usize, usize>) -> String {
    let d = &c.docs[p.idx];
    let fm = d.fm();
    let get = |k: &str| fm.and_then(|m| m.get(k));
    let mut lines = vec!["---".to_string()];
    if t == Target::Docusaurus {
        let stem = p.file.rsplit('/').next().unwrap().trim_end_matches(".md");
        lines.push(format!("id: {}", yaml_str(stem)));
        lines.push(format!("specfold_id: {}", yaml_str(&d.id.to_string())));
        lines.push(format!("sidebar_label: {}", yaml_str(&p.title)));
        lines.push(format!("sidebar_position: {}", by_idx[&p.idx] + 1));
    } else {
        lines.push(format!("id: {}", yaml_str(&d.id.to_string())));
    }
    lines.push(format!("type: {}", yaml_str(&d.id.ty)));
    if let Some(s) = d.status() {
        lines.push(format!("status: {}", yaml_str(&s)));
    }
    lines.push(format!("title: {}", yaml_str(&p.title)));
    for k in ["aliases", "tags"] {
        let v = strings(get(k));
        if !v.is_empty() {
            lines.push(format!("{k}: {}", yaml_list(&v)));
        }
    }
    for k in ["created", "updated"] {
        if let Some(v) = get(k).and_then(|v| v.as_str()) {
            lines.push(format!("{k}: {}", yaml_str(v)));
        }
    }
    if let Some(Value::Object(rel)) = get("rel") {
        for (kind, v) in rel {
            let ids: Vec<String> = strings(Some(v))
                .into_iter()
                .map(|s| match (t, g_target(c, &s, pages, by_idx)) {
                    (Target::Obsidian, Some(q)) => format!("[[{}|{}]]", q.file.trim_end_matches(".md"), q.title),
                    _ => s,
                })
                .collect();
            if !ids.is_empty() {
                lines.push(format!("rel_{}: {}", kind.replace(['-', '.'], "_"), yaml_list(&ids)));
            }
        }
    }
    lines.push("---".into());
    lines.join("\n") + "\n\n"
}

/// The exported page for a relation target written as an id.
fn g_target<'a>(c: &Corpus, id: &str, pages: &'a [Page], by_idx: &HashMap<usize, usize>) -> Option<&'a Page> {
    let target = crate::model::RefText::parse(id).target;
    let idx = c.docs.iter().position(|d| d.id.to_string() == target)?;
    by_idx.get(&idx).map(|&i| &pages[i])
}

/// GitHub's heading slug: lower case, punctuation dropped, spaces to `-`.
pub fn github_slug(heading: &str) -> String {
    heading
        .trim()
        .to_lowercase()
        .chars()
        .filter_map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                Some(c)
            } else if c == ' ' {
                Some('-')
            } else {
                None
            }
        })
        .collect()
}

/// A relative link from `from` to `to`, both tree paths.
fn relative(from: &str, to: &str) -> String {
    let from_dir: Vec<&str> = from.split('/').collect::<Vec<_>>()[..from.matches('/').count()].to_vec();
    let to_parts: Vec<&str> = to.split('/').collect();
    let common = from_dir.iter().zip(&to_parts).take_while(|(a, b)| a == b).count();
    let mut out: Vec<String> = vec!["..".into(); from_dir.len() - common];
    out.extend(to_parts[common..].iter().map(|s| s.to_string()));
    out.join("/")
}

fn rewrite(
    p: &Page,
    t: Target,
    pages: &[Page],
    by_idx: &HashMap<usize, usize>,
    unresolved: &mut Vec<Unresolved>,
    src: &str,
) -> String {
    let mut out = String::new();
    let mut at = 0;
    for l in &p.links {
        out.push_str(&p.body[at..l.span.0]);
        at = l.span.1;
        let dest = l.idx.and_then(|i| by_idx.get(&i)).map(|&i| &pages[i]);
        let text = l.label.clone().or_else(|| dest.map(|q| q.title.clone())).or_else(|| l.title.clone());
        let Some(q) = dest.filter(|_| l.reason.is_none()) else {
            let reason = l.reason.clone().unwrap_or_else(|| {
                if l.remote.is_some() {
                    format!("in remote {}", l.remote.clone().unwrap_or_default())
                } else {
                    "not part of this export".into()
                }
            });
            unresolved.push(Unresolved { path: src.into(), raw: l.raw.clone(), reason });
            let inner = l.raw.trim_start_matches("[[").trim_end_matches("]]");
            out.push_str(&text.unwrap_or_else(|| inner.split('|').next().unwrap_or(inner).trim().to_string()));
            continue;
        };
        let text = text.unwrap_or_else(|| q.title.clone());
        let heading = l.anchor.as_ref().map(|a| q.headings.get(a).cloned().unwrap_or_else(|| a.clone()));
        out.push_str(&match t {
            Target::Obsidian => {
                let h = heading.map(|h| format!("#{h}")).unwrap_or_default();
                format!("[[{}{h}|{text}]]", q.file.trim_end_matches(".md"))
            }
            Target::Mkdocs | Target::Docusaurus => {
                let a = l.anchor.as_ref().map(|a| format!("#{a}")).unwrap_or_default();
                format!("[{text}]({}{a})", relative(&p.file, &q.file))
            }
            Target::GithubWiki => {
                let a = heading.map(|h| format!("#{}", github_slug(&h))).unwrap_or_default();
                format!("[{text}]({}{a})", q.file.trim_end_matches(".md"))
            }
        });
    }
    out.push_str(&p.body[at..]);
    if t.keeps_anchors() { out } else { strip_anchors(&out) }
}

/// Removes `{#anchor}` from headings outside code fences.
fn strip_anchors(text: &str) -> String {
    let re = regex::Regex::new(r"\s*\{#[A-Za-z0-9][A-Za-z0-9_.-]*\}\s*$").unwrap();
    let mut fence = false;
    let mut out = String::new();
    for line in text.split_inclusive('\n') {
        let tr = line.trim_start();
        if tr.starts_with("```") || tr.starts_with("~~~") {
            fence = !fence;
        }
        if !fence && tr.starts_with('#') {
            let nl = if line.ends_with('\n') { "\n" } else { "" };
            out.push_str(&re.replace(line.trim_end_matches(['\n', '\r']), ""));
            out.push_str(nl);
        } else {
            out.push_str(line);
        }
    }
    out
}

fn index_files(tree: &mut Tree, c: &Corpus, t: Target, pages: &[Page]) {
    let mut by_type: BTreeMap<&str, Vec<&Page>> = BTreeMap::new();
    for p in pages {
        by_type.entry(c.docs[p.idx].id.ty.as_str()).or_default().push(p);
    }
    let list = |link: &dyn Fn(&Page) -> String| {
        let mut s = String::new();
        for (ty, ps) in &by_type {
            s.push_str(&format!("\n## {ty}\n\n"));
            for p in ps {
                s.push_str(&format!("- {}\n", link(p)));
            }
        }
        s
    };
    match t {
        Target::GithubWiki => {
            let link = |p: &Page| format!("[{}]({})", p.title, p.file.trim_end_matches(".md"));
            tree.files.insert("Home.md".into(), format!("# Specs\n{}", list(&link)));
            tree.files.insert("_Sidebar.md".into(), list(&link).trim_start().to_string());
        }
        Target::Mkdocs | Target::Docusaurus => {
            let link = |p: &Page| format!("[{}]({})", p.title, p.file);
            tree.files.insert("index.md".into(), format!("# Specs\n{}", list(&link)));
        }
        Target::Obsidian => {}
    }
}

/// Whether `out` may be written: missing, empty, or holding a previous export's manifest.
/// Returns the files the previous export wrote.
pub fn check_out(out: &Path, root: &Path) -> Result<Vec<String>> {
    if out.components().any(|c| matches!(c.as_os_str().to_str(), Some(".spec" | ".spec.local" | ".git"))) {
        bail!("refusing to export into {}: it is inside a spec or git directory", out.display());
    }
    if !out.exists() {
        return Ok(vec![]);
    }
    if !out.is_dir() {
        bail!("{} is not a directory", out.display());
    }
    let abs = out.canonicalize()?;
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    if root.starts_with(&abs) {
        bail!("refusing to export into {}: it contains the repository", out.display());
    }
    let m = out.join(MANIFEST);
    if m.is_file() {
        let v: Value =
            serde_json::from_str(&std::fs::read_to_string(&m)?).with_context(|| format!("reading {}", m.display()))?;
        return Ok(v["files"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|f| f.as_str().map(str::to_string))
            .collect());
    }
    if std::fs::read_dir(out)?.next().is_some() {
        bail!(
            "refusing to export into {}: it is not empty and holds no {MANIFEST} from an earlier export",
            out.display()
        );
    }
    Ok(vec![])
}

/// Writes the tree and its manifest, removing the files of the previous export first.
pub fn write(out: &Path, tree: &Tree, target: Target, previous: &[String]) -> Result<()> {
    for f in previous {
        if f.split('/').any(|s| s == ".." || s.is_empty()) || Path::new(f).is_absolute() {
            continue;
        }
        let p = out.join(f);
        if p.is_file() {
            std::fs::remove_file(&p)?;
            let mut dir = p.parent();
            while let Some(d) = dir.filter(|d| *d != out) {
                if std::fs::remove_dir(d).is_err() {
                    break;
                }
                dir = d.parent();
            }
        }
    }
    for (rel, text) in &tree.files {
        let p = out.join(rel);
        std::fs::create_dir_all(p.parent().unwrap())?;
        std::fs::write(&p, text)?;
    }
    let manifest = json!({
        "generator": "specfold",
        "target": target.name(),
        "files": tree.files.keys().collect::<Vec<_>>(),
    });
    std::fs::write(out.join(MANIFEST), serde_json::to_string_pretty(&manifest)? + "\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_paths() {
        assert_eq!(safe_name("a:b/c"), "a-b-c");
        assert_eq!(relative("shop/adr/a.md", "shop/sot/b.md"), "../sot/b.md");
        assert_eq!(relative("shop/adr/a.md", "shop/adr/b.md"), "b.md");
        assert_eq!(github_slug("Cart Totals (v2)"), "cart-totals-v2");
        assert_eq!(strip_anchors("## Totals {#CO-1}\n```\n## x {#y}\n```\n"), "## Totals\n```\n## x {#y}\n```\n");
    }
}
