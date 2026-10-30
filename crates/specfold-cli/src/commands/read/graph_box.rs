//! `spec graph --format box`: a left-to-right layered drawing of the document graph.
//!
//! Layers are longest paths from the documents nothing points at; an edge that skips layers
//! runs through a line in each layer between; a cycle edge is listed under the drawing.

// Grid drawing indexes rows and columns by coordinate on purpose.
#![allow(clippy::needless_range_loop)]

use std::collections::BTreeMap;

/// The largest graph drawn as boxes; past this the tree reads better.
pub const MAX_NODES: usize = 60;
const MIN_WIDTH: usize = 16;
const U: u8 = 1;
const D: u8 = 2;
const L: u8 = 4;
const R: u8 = 8;

pub struct Drawing {
    pub lines: Vec<String>,
    pub width: usize,
}

/// Why a drawing was not made.
pub enum Skip {
    TooMany(usize),
    TooWide { need: usize, have: usize },
}

/// `labels[i]` names node i (lines split on '\\n', two at most); `edges` are (from, to, kind). `depth` keeps layers 0..=depth.
pub fn draw(
    labels: &[String],
    edges: &[(usize, usize, String)],
    depth: Option<usize>,
    columns: usize,
    ascii: bool,
) -> Result<Drawing, Skip> {
    let n = labels.len();
    let layer = layers_of(n, edges);
    let keep: Vec<bool> = (0..n).map(|i| depth.is_none_or(|d| layer[i] <= d)).collect();
    let shown = keep.iter().filter(|k| **k).count();
    if shown > MAX_NODES {
        return Err(Skip::TooMany(shown));
    }
    let (mut forward, mut back) = (vec![], vec![]);
    for (f, t, k) in edges {
        if !keep[*f] || !keep[*t] {
            continue;
        }
        if layer[*t] > layer[*f] { forward.push((*f, *t, k.clone())) } else { back.push((*f, *t, k.clone())) }
    }
    let layers = (0..n).filter(|i| keep[*i]).map(|i| layer[i] + 1).max().unwrap_or(0);

    // Slots: real nodes, then a pass-through slot per layer an edge skips.
    // slot = (node or None, kind, ...); paths[e] = slot ids per layer from source to target.
    let (slots, hops) = slots_of(&layer, &keep, &forward);
    let (row, cols) = order(labels, &slots, &hops, layers);

    // Widths: one box width for all, each gap has a lane per source slot.
    let lanes: Vec<Vec<usize>> = (0..layers)
        .map(|l| {
            let mut src: Vec<usize> = hops.iter().filter(|h| slots[h.0].0 == l).map(|h| h.0).collect();
            src.sort_by(|a, b| row[*a].partial_cmp(&row[*b]).expect("row positions are finite"));
            src.dedup();
            src
        })
        .collect();
    let gaps: Vec<usize> = lanes.iter().map(|s| if s.is_empty() { 0 } else { 4 + s.len() }).collect();
    let gap_sum: usize = gaps.iter().take(layers.saturating_sub(1)).sum();
    let longest =
        (0..n).filter(|i| keep[*i]).flat_map(|i| labels[i].lines().map(|l| l.chars().count())).max().unwrap_or(0);
    let room = columns.saturating_sub(gap_sum) / layers.max(1);
    let w = (longest + 4).min(room);
    if w < MIN_WIDTH.min(longest + 4) {
        return Err(Skip::TooWide { need: gap_sum + layers * MIN_WIDTH.min(longest + 4), have: columns });
    }
    let mut x0 = vec![0usize; layers];
    for l in 1..layers {
        x0[l] = x0[l - 1] + w + gaps[l - 1];
    }
    let width = x0.last().map_or(0, |x| x + w);
    let height = cols.iter().map(|c| c.len()).max().unwrap_or(0) * 5;
    let mut ch = vec![vec![' '; width]; height];
    let mut mask = vec![vec![0u8; width]; height];
    let mid = |s: usize| row[s] as usize * 5 + 1;
    let line = |mask: &mut Vec<Vec<u8>>, y: usize, a: usize, b: usize| {
        for x in a.min(b)..=a.max(b) {
            let m = if x == a.min(b) {
                R
            } else if x == a.max(b) {
                L
            } else {
                L | R
            };
            mask[y][x] |= m;
        }
    };
    let arrow = if ascii { '>' } else { '▶' };
    for (s, (l, node)) in slots.iter().enumerate() {
        let (x, y) = (x0[*l], mid(s));
        match node {
            Some(i) => draw_box(&mut ch, &labels[*i], x, y, w, ascii),
            None => line(&mut mask, y, x, x + w - 1),
        }
    }
    let mut marks: BTreeMap<(usize, usize), char> = BTreeMap::new();
    let mut kinds: BTreeMap<char, String> = BTreeMap::new();
    for (a, b, kind) in &hops {
        let l = slots[*a].0;
        let lane = x0[l] + w + 1 + lanes[l].iter().position(|s| s == a).expect("edge source sits in its lane");
        let (ya, yb, xb) = (mid(*a), mid(*b), x0[l + 1]);
        line(&mut mask, ya, x0[l] + w, lane);
        for y in ya.min(yb)..=ya.max(yb) {
            mask[y][lane] |= if y == ya.min(yb) {
                D
            } else if y == ya.max(yb) {
                U
            } else {
                U | D
            };
        }
        if ya == yb {
            mask[ya][lane] &= !(U | D);
        }
        line(&mut mask, yb, lane, xb - 1);
        if let Some(k) = kind {
            let c = k.chars().next().unwrap_or('?');
            let seen = kinds.entry(c).or_insert_with(|| k.clone());
            let c = if seen == k { c } else { '*' };
            let at = marks.entry((yb, xb - 2)).or_insert(c);
            if *at != c {
                *at = '*';
            }
            marks.insert((yb, xb - 1), arrow);
        }
    }
    for y in 0..height {
        for x in 0..width {
            let m = mask[y][x];
            if m != 0 {
                ch[y][x] = junction(m, ascii);
            }
        }
    }
    for ((y, x), c) in &marks {
        ch[*y][*x] = *c;
    }
    let mut lines: Vec<String> =
        ch.into_iter().map(|r| r.into_iter().collect::<String>().trim_end().to_string()).collect();
    lines.retain(|l| !l.is_empty());
    if !kinds.is_empty() {
        let mut legend: Vec<String> = kinds.iter().map(|(c, k)| format!("{c} {k}")).collect();
        if marks.values().any(|c| *c == '*') {
            legend.push("* more than one".to_string());
        }
        lines.push(legend.join(", "));
    }
    for (f, t, k) in &back {
        lines.push(format!("also: {} {k} {} (a cycle)", first_line(&labels[*f]), first_line(&labels[*t])));
    }
    Ok(Drawing { lines, width })
}

/// Slots: real nodes, then a pass-through slot per layer an edge skips; and the hops between them.
fn slots_of(
    layer: &[usize],
    keep: &[bool],
    forward: &[(usize, usize, String)],
) -> (Vec<(usize, Option<usize>)>, Vec<Hop>) {
    let mut slots: Vec<(usize, Option<usize>)> =
        (0..layer.len()).filter(|i| keep[*i]).map(|i| (layer[i], Some(i))).collect();
    let node_slot: BTreeMap<usize, usize> =
        slots.iter().enumerate().filter_map(|(s, (_, i))| i.map(|i| (i, s))).collect();
    let mut hops: Vec<(usize, usize, Option<String>)> = vec![]; // (from slot, to slot, label on the last hop)
    for (f, t, k) in forward {
        let mut prev = node_slot[f];
        for l in layer[*f] + 1..layer[*t] {
            slots.push((l, None));
            hops.push((prev, slots.len() - 1, None));
            prev = slots.len() - 1;
        }
        hops.push((prev, node_slot[t], Some(k.clone())));
    }
    (slots, hops)
}

/// Longest-path layers; n passes bound a cycle.
fn layers_of(n: usize, edges: &[(usize, usize, String)]) -> Vec<usize> {
    let mut layer = vec![0usize; n];
    for _ in 0..n {
        let mut moved = false;
        for (f, t, _) in edges {
            if layer[*t] < layer[*f] + 1 && layer[*f] + 1 < n {
                layer[*t] = layer[*f] + 1;
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    layer
}

/// An edge segment: (from slot, to slot, label on the last hop).
type Hop = (usize, usize, Option<String>);

/// Orders each layer by the mean row of what feeds it, ids breaking ties: (row per slot, slots per layer).
fn order(
    labels: &[String],
    slots: &[(usize, Option<usize>)],
    hops: &[Hop],
    layers: usize,
) -> (Vec<f64>, Vec<Vec<usize>>) {
    let mut row = vec![0f64; slots.len()];
    let mut cols: Vec<Vec<usize>> = vec![vec![]; layers];
    for l in 0..layers {
        let mut members: Vec<usize> = (0..slots.len()).filter(|s| slots[*s].0 == l).collect();
        let key = |s: usize, row: &[f64]| {
            let ins: Vec<f64> = hops.iter().filter(|h| h.1 == s).map(|h| row[h.0]).collect();
            if ins.is_empty() { f64::MAX } else { ins.iter().sum::<f64>() / ins.len() as f64 }
        };
        members.sort_by(|a, b| {
            key(*a, &row).partial_cmp(&key(*b, &row)).expect("barycenter keys are finite").then_with(|| {
                let name = |s: usize| slots[s].1.map(|i| labels[i].clone()).unwrap_or_default();
                name(*a).cmp(&name(*b))
            })
        });
        for (r, s) in members.iter().enumerate() {
            row[*s] = r as f64;
        }
        cols[l] = members;
    }
    (row, cols)
}

/// One node's box, its top-left corner one row above `y`.
fn draw_box(ch: &mut [Vec<char>], label: &str, x: usize, y: usize, w: usize, ascii: bool) {
    let (h, v, tl, tr, bl, br, more) =
        if ascii { ('-', '|', '+', '+', '+', '+', '~') } else { ('─', '│', '┌', '┐', '└', '┘', '…') };
    let inner = w - 4;
    let fit = |t: &str| {
        if t.chars().count() > inner {
            t.chars().take(inner - 1).collect::<String>() + &more.to_string()
        } else {
            t.to_string()
        }
    };
    let mut text = label.lines();
    let rows = [fit(text.next().unwrap_or("")), fit(text.next().unwrap_or(""))];
    ch[y - 1][x] = tl;
    ch[y - 1][x + w - 1] = tr;
    ch[y + 2][x] = bl;
    ch[y + 2][x + w - 1] = br;
    for c in 1..w - 1 {
        ch[y - 1][x + c] = h;
        ch[y + 2][x + c] = h;
    }
    for (r, t) in rows.iter().enumerate() {
        for (c, k) in format!("{v} {t:<inner$} {v}").chars().enumerate() {
            ch[y + r][x + c] = k;
        }
    }
}

fn junction(m: u8, ascii: bool) -> char {
    if ascii {
        return match m {
            x if x & (U | D) == 0 => '-',
            x if x & (L | R) == 0 => '|',
            _ => '+',
        };
    }
    match m {
        x if x & (U | D) == 0 => '─',
        x if x & (L | R) == 0 => '│',
        x if x == R | D => '┌',
        x if x == L | D => '┐',
        x if x == R | U => '└',
        x if x == L | U => '┘',
        x if x == L | R | D => '┬',
        x if x == L | R | U => '┴',
        x if x == U | D | R => '├',
        x if x == U | D | L => '┤',
        _ => '┼',
    }
}

/// C131: a box label is the id over the status; a one-line note wants only the id.
fn first_line(label: &str) -> &str {
    label.lines().next().unwrap_or(label)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn layers_run_left_to_right_and_skipped_layers_pass_through() {
        let labels = s(&["plan:a", "page:b", "sot:c"]);
        let edges = vec![(0, 1, "implements".into()), (1, 2, "depends_on".into()), (0, 2, "implements".into())];
        let d = draw(&labels, &edges, None, 200, false).ok().unwrap();
        let text = d.lines.join("\n");
        let col = |t: &str| d.lines.iter().find_map(|l| l.find(t)).unwrap();
        assert!(col("plan:a") < col("page:b") && col("page:b") < col("sot:c"), "{text}");
        assert!(text.contains("i▶") && text.contains("*▶"), "two kinds into one box show *: {text}");
        assert!(text.ends_with("d depends_on, i implements, * more than one"), "{text}");
        let a = draw(&labels, &edges, None, 200, true).ok().unwrap().lines.join("\n");
        assert!(a.is_ascii() && a.contains("i>"), "{a}");
    }

    #[test]
    fn narrow_terminals_shrink_boxes_then_refuse_and_cycles_are_listed() {
        let labels = s(&["adr:a-very-long-document-name", "sot:b"]);
        let edges = vec![(0, 1, "supersedes".into()), (1, 0, "evidence".into())];
        let d = draw(&labels, &edges, None, 50, false).ok().unwrap();
        assert!(d.width <= 50 && d.lines.join("\n").contains('…'), "{}", d.lines.join("\n"));
        assert!(d.lines.iter().any(|l| l.starts_with("also: ")), "{}", d.lines.join("\n"));
        assert!(matches!(draw(&labels, &edges, None, 20, false), Err(Skip::TooWide { .. })));
        let many: Vec<String> = (0..MAX_NODES + 1).map(|i| format!("sot:{i}")).collect();
        assert!(matches!(draw(&many, &[], None, 500, false), Err(Skip::TooMany(_))));
    }
}
