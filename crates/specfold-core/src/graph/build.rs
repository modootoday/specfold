//! Builds the graph: indexes ids, paths and aliases, then resolves relations and links into edges.

use super::types::{DOC_RELATIONS, Edge, Graph, MENTIONS, Resolution};
use crate::corpus::Corpus;
use crate::model::RefText;

impl Graph {
    pub fn build(c: &Corpus) -> Self {
        let mut g = Graph {
            anchors: vec![Default::default(); c.docs.len()],
            outgoing: vec![vec![]; c.docs.len()],
            incoming: vec![vec![]; c.docs.len()],
            ..Default::default()
        };
        for (i, d) in c.docs.iter().enumerate() {
            g.by_id.insert(d.id.to_string(), i);
            g.by_path.insert(d.path.clone(), i);
            if let Some(dir) = &d.dir {
                g.by_path.insert(dir.clone(), i);
            }
            for a in d.list_field("aliases") {
                g.by_alias.entry(a).or_default().push(i);
            }
            g.anchors[i] = d.anchors.iter().map(|a| a.item.clone()).collect();
        }
        for (old, news) in &c.aliases.map {
            for n in news {
                if let Some(&i) = g.by_id.get(n) {
                    let list = g.by_alias.entry(old.clone()).or_default();
                    if !list.contains(&i) {
                        list.push(i);
                    }
                }
            }
        }
        for (i, d) in c.docs.iter().enumerate() {
            for (kind, targets) in d.rel() {
                if !DOC_RELATIONS.contains(&kind.as_str()) {
                    continue;
                }
                for t in targets {
                    g.link(i, &t, &kind);
                }
            }
            for l in &d.links {
                g.link(i, &l.item, MENTIONS);
            }
        }
        g
    }

    fn link(&mut self, from: usize, text: &str, kind: &str) {
        if let Resolution::Doc { idx, anchor, .. } = self.resolve(text) {
            if idx == from {
                return;
            }
            let e = self.edges.len();
            self.edges.push(Edge { from, to: idx, kind: kind.to_string(), anchor });
            self.outgoing[from].push(e);
            self.incoming[idx].push(e);
        }
    }

    /// Resolves an id, an alias, or a path to a document, checking the anchor if there is one.
    pub fn resolve(&self, text: &str) -> Resolution {
        let r = RefText::parse(text);
        // A path written relative to the citing file (`../../.spec/…`) names the same document;
        // ids never start with a dot, so stripping these prefixes cannot change an id.
        let mut target = r.target.trim_end_matches('/');
        while let Some(rest) = target.strip_prefix("../").or_else(|| target.strip_prefix("./")) {
            target = rest;
        }
        let (candidates, via_alias) = if let Some(&i) = self.by_id.get(target).or_else(|| self.by_path.get(target)) {
            (vec![i], false)
        } else if let Some(list) = self.by_alias.get(target) {
            (list.clone(), true)
        } else {
            (vec![], false)
        };
        match candidates.as_slice() {
            [] => Resolution::Missing,
            [idx] => match r.anchor {
                Some(a) if !self.anchors[*idx].contains(&a) => Resolution::MissingAnchor { idx: *idx, anchor: a },
                anchor => Resolution::Doc { idx: *idx, anchor, via_alias },
            },
            many => Resolution::Ambiguous(many.to_vec()),
        }
    }

    pub fn edges_from(&self, idx: usize) -> impl Iterator<Item = &Edge> {
        self.outgoing[idx].iter().map(|&e| &self.edges[e])
    }

    pub fn edges_to(&self, idx: usize) -> impl Iterator<Item = &Edge> {
        self.incoming[idx].iter().map(|&e| &self.edges[e])
    }
}
