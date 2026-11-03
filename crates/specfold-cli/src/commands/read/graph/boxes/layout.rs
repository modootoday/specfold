//! Where a box drawing puts things: longest-path layers, pass-through slots for edges that skip a
//! layer, the order within each layer, and the columns each layer and lane take.

// Layout indexes layers and slots by position on purpose.
#![allow(clippy::needless_range_loop)]

use super::draw::Skip;
use std::collections::BTreeMap;

const MIN_WIDTH: usize = 16;

/// Longest-path layers; n passes bound a cycle.
pub(super) fn layers_of(n: usize, edges: &[(usize, usize, String)]) -> Vec<usize> {
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

/// Slots: real nodes, then a pass-through slot per layer an edge skips; and the hops between them.
pub(super) fn slots_of(
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

/// An edge segment: (from slot, to slot, label on the last hop).
pub(super) type Hop = (usize, usize, Option<String>);

/// Orders each layer by the mean row of what feeds it, ids breaking ties: (row per slot, slots per layer).
pub(super) fn order(
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

/// Where everything goes: a lane per source slot in each gap, one box width, each layer's left
/// edge, and the size of the grid.
pub(super) struct Layout {
    pub(super) lanes: Vec<Vec<usize>>,
    pub(super) row: Vec<f64>,
    pub(super) x0: Vec<usize>,
    pub(super) w: usize,
    pub(super) width: usize,
    pub(super) height: usize,
}

impl Layout {
    /// Widths: one box width for all, each gap has a lane per source slot.
    pub(super) fn fit(
        slots: &[(usize, Option<usize>)],
        hops: &[Hop],
        row: &[f64],
        cols: &[Vec<usize>],
        layers: usize,
        columns: usize,
        longest: usize,
    ) -> Result<Layout, Skip> {
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
        Ok(Layout { lanes, row: row.to_vec(), x0, w, width, height })
    }

    /// The text row through the middle of slot `s`.
    pub(super) fn mid(&self, s: usize) -> usize {
        self.row[s] as usize * 5 + 1
    }
}
