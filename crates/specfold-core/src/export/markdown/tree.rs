//! Building the page tree in memory: one page per exported document, and the index files.

use super::front::{Index, front};
use super::page::Page;
use super::rewrite::rewrite;
use super::types::{Options, Target, Tree};
use crate::corpus::Corpus;
use crate::export::links;
use crate::graph::Graph;
use crate::markdown::{body, frontmatter};
use crate::model::{Tier, stamp::slugify};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Characters no target or file system accepts in a file name.
pub(super) fn safe_name(s: &str) -> String {
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
    // Relation targets are looked up per page; a scan of every document each time made export
    // quadratic. The first document with an id wins, as the scan did.
    let mut by_id: HashMap<String, usize> = HashMap::new();
    for (i, d) in c.docs.iter().enumerate() {
        by_id.entry(d.id.to_string()).or_insert(i);
    }
    let ix = Index { pages: &pages, by_idx: &by_idx, by_id: &by_id };
    for p in &pages {
        let mut out = String::new();
        if !o.target.flat() {
            out.push_str(&front(c, p, o.target, &ix));
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
