//! Groups a root's files into documents, and builds each document from the scans of its files.

use super::location::{Location, Placed, Role, locate};
use super::scan::FileScan;
use super::types::{SpecRoot, Stray};
use crate::config::Config;
use crate::model::{Doc, DocId, Located};
use std::collections::{BTreeMap, HashMap};

/// The files of one document before they are read.
pub struct Pending {
    pub root: SpecRoot,
    pub main: String,
    pub placed: Placed,
    pub chapters: Vec<String>,
    pub attachments: Vec<String>,
}

#[derive(Default)]
struct Group {
    placed: Option<Placed>,
    main: Option<String>,
    chapters: Vec<String>,
    attachments: Vec<String>,
}

/// Sorts `files` of one root into documents; paths that fit nowhere become strays.
pub fn group(cfg: &Config, root: &SpecRoot, files: &[String], strays: &mut Vec<Stray>) -> Vec<Pending> {
    let alias_file = cfg.raw.aliases.file.as_deref();
    let mut groups: BTreeMap<(String, String), Group> = BTreeMap::new();
    for path in files {
        if Some(path.as_str()) == alias_file {
            continue;
        }
        let rest = path.strip_prefix(&format!("{}/", root.dir)).unwrap_or(path);
        match locate(&cfg.registry, rest) {
            Location::Stray(reason) => strays.push(Stray { path: path.clone(), reason }),
            Location::Doc(p) => {
                let g = groups.entry((p.ty.clone(), p.stem.clone())).or_default();
                match p.role {
                    Role::Main => g.main = Some(path.clone()),
                    Role::Chapter => g.chapters.push(path.clone()),
                    Role::Attachment => g.attachments.push(path.clone()),
                }
                g.placed.get_or_insert(p);
            }
        }
    }
    let mut out = vec![];
    for g in groups.into_values() {
        match (g.main, g.placed) {
            (Some(main), Some(placed)) => {
                out.push(Pending { root: root.clone(), main, placed, chapters: g.chapters, attachments: g.attachments })
            }
            (None, Some(p)) => {
                for f in g.chapters.into_iter().chain(g.attachments) {
                    strays.push(Stray { path: f, reason: format!("bundle has no index.{}.md", p.ty) });
                }
            }
            _ => {}
        }
    }
    out
}

fn located<'a>(path: &str, items: impl Iterator<Item = (usize, &'a String)>) -> Vec<Located<String>> {
    items.map(|(line, item)| Located { path: path.to_string(), line, item: item.clone() }).collect()
}

/// Builds a document from the scans of its main file and chapters, taking them out of `scans`.
pub fn build(p: Pending, scans: &mut HashMap<String, FileScan>) -> Option<Doc> {
    let main = scans.remove(&p.main)?;
    let mut links = located(&p.main, main.body.links.iter().map(|l| (l.line, &l.text)));
    let mut anchors = located(&p.main, main.body.anchors.iter().map(|a| (a.line, &a.name)));
    let mut path_cites = located(&p.main, main.body.path_cites.iter().map(|l| (l.line, &l.text)));
    let mut body_lines = main.body.lines;
    for ch in &p.chapters {
        let Some(s) = scans.remove(ch) else { continue };
        links.extend(located(ch, s.body.links.iter().map(|l| (l.line, &l.text))));
        anchors.extend(located(ch, s.body.anchors.iter().map(|a| (a.line, &a.name))));
        path_cites.extend(located(ch, s.body.path_cites.iter().map(|l| (l.line, &l.text))));
        body_lines += s.body.lines;
    }
    let title = main
        .frontmatter
        .map
        .as_ref()
        .and_then(|m| m.get("title"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or(main.body.first_heading);
    let dir = p.placed.bundle.then(|| p.main.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default());
    Some(Doc {
        id: DocId::new(&p.placed.ty, &p.root.owner, &p.placed.key),
        dir,
        tier: p.root.tier,
        slug: p.placed.slug,
        frontmatter: main.frontmatter,
        title,
        chapters: p.chapters,
        bundle_files: p.attachments,
        links,
        anchors,
        path_cites,
        body_lines,
        body_hash: main.body_hash,
        path: p.main,
    })
}
