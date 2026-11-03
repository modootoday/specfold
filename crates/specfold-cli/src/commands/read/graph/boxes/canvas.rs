//! Painting a box drawing on a character grid: boxes, edge lines joined at their crossings, and
//! the marks and legend for relation kinds.

// Grid drawing indexes rows and columns by coordinate on purpose.
#![allow(clippy::needless_range_loop)]

use super::layout::{Hop, Layout};
use std::collections::BTreeMap;

const U: u8 = 1;
const D: u8 = 2;
const L: u8 = 4;
const R: u8 = 8;

/// The grid being drawn: characters, the line directions meeting in each cell, and the marks
/// (relation letters and arrow heads) placed over them.
pub(super) struct Canvas {
    pub(super) ch: Vec<Vec<char>>,
    pub(super) mask: Vec<Vec<u8>>,
    pub(super) marks: BTreeMap<(usize, usize), char>,
    pub(super) kinds: BTreeMap<char, String>,
    pub(super) ascii: bool,
}

impl Canvas {
    pub(super) fn new(layout: &Layout, ascii: bool) -> Canvas {
        Canvas {
            ch: vec![vec![' '; layout.width]; layout.height],
            mask: vec![vec![0u8; layout.width]; layout.height],
            marks: BTreeMap::new(),
            kinds: BTreeMap::new(),
            ascii,
        }
    }

    /// A horizontal line on row `y` from column `a` to `b`.
    pub(super) fn line(&mut self, y: usize, a: usize, b: usize) {
        for x in a.min(b)..=a.max(b) {
            let m = if x == a.min(b) {
                R
            } else if x == a.max(b) {
                L
            } else {
                L | R
            };
            self.mask[y][x] |= m;
        }
    }

    /// One edge segment: out of its source box, down or up its lane, into the next layer; the
    /// last segment of an edge gets its relation letter and an arrow head.
    pub(super) fn hop(&mut self, layout: &Layout, slots: &[(usize, Option<usize>)], (a, b, kind): &Hop) {
        let (x0, w) = (&layout.x0, layout.w);
        let l = slots[*a].0;
        let lane = x0[l] + w + 1 + layout.lanes[l].iter().position(|s| s == a).expect("edge source sits in its lane");
        let (ya, yb, xb) = (layout.mid(*a), layout.mid(*b), x0[l + 1]);
        self.line(ya, x0[l] + w, lane);
        for y in ya.min(yb)..=ya.max(yb) {
            self.mask[y][lane] |= if y == ya.min(yb) {
                D
            } else if y == ya.max(yb) {
                U
            } else {
                U | D
            };
        }
        if ya == yb {
            self.mask[ya][lane] &= !(U | D);
        }
        self.line(yb, lane, xb - 1);
        if let Some(k) = kind {
            let c = k.chars().next().unwrap_or('?');
            let seen = self.kinds.entry(c).or_insert_with(|| k.clone());
            let c = if seen == k { c } else { '*' };
            let at = self.marks.entry((yb, xb - 2)).or_insert(c);
            if *at != c {
                *at = '*';
            }
            let arrow = if self.ascii { '>' } else { '▶' };
            self.marks.insert((yb, xb - 1), arrow);
        }
    }

    /// The finished rows, blank ones dropped, and the legend of relation letters.
    pub(super) fn lines(mut self) -> Vec<String> {
        for y in 0..self.ch.len() {
            for x in 0..self.ch[y].len() {
                let m = self.mask[y][x];
                if m != 0 {
                    self.ch[y][x] = junction(m, self.ascii);
                }
            }
        }
        for ((y, x), c) in &self.marks {
            self.ch[*y][*x] = *c;
        }
        let mut lines: Vec<String> =
            self.ch.into_iter().map(|r| r.into_iter().collect::<String>().trim_end().to_string()).collect();
        lines.retain(|l| !l.is_empty());
        if !self.kinds.is_empty() {
            let mut legend: Vec<String> = self.kinds.iter().map(|(c, k)| format!("{c} {k}")).collect();
            if self.marks.values().any(|c| *c == '*') {
                legend.push("* more than one".to_string());
            }
            lines.push(legend.join(", "));
        }
        lines
    }
}

/// One node's box, its top-left corner one row above `y`.
pub(super) fn draw_box(ch: &mut [Vec<char>], label: &str, x: usize, y: usize, w: usize, ascii: bool) {
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
