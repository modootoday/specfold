//! The changed regions between two texts, as a unified diff splits them.

/// One changed region: where it starts in the old and new text (1-based) and its lines.
pub(super) struct Hunk {
    pub(super) old_at: usize,
    pub(super) new_at: usize,
    pub(super) before: Vec<String>,
    pub(super) after: Vec<String>,
}

/// The changed regions between two texts, as a unified diff splits them: changes with an
/// unchanged line between them are separate hunks, so a refreshed `updated:` line and an edited
/// section far below it are shown apart rather than as one block spanning both.
pub(super) fn hunks(old: &str, new: &str) -> Vec<Hunk> {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    // Common start and end first: the table below is then only as big as the changed middle.
    let head = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let tail = a[head..].iter().rev().zip(b[head..].iter().rev()).take_while(|(x, y)| x == y).count();
    let (x, y) = (&a[head..a.len() - tail], &b[head..b.len() - tail]);
    // C158: the table below grows with the product of the two sides. Past MAX_CELLS the middle is
    // shown as one hunk, which needs no table; the write is the same either way.
    if x.len().saturating_mul(y.len()) > MAX_CELLS {
        return vec![Hunk::new(
            head,
            head,
            x.iter().map(|s| s.to_string()).collect(),
            y.iter().map(|s| s.to_string()).collect(),
        )];
    }
    // Longest common subsequence over the middle.
    let mut l = vec![vec![0u32; y.len() + 1]; x.len() + 1];
    for i in (0..x.len()).rev() {
        for j in (0..y.len()).rev() {
            l[i][j] = if x[i] == y[j] { l[i + 1][j + 1] + 1 } else { l[i + 1][j].max(l[i][j + 1]) };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut out: Vec<Hunk> = vec![];
    let mut open = false;
    while i < x.len() || j < y.len() {
        if i < x.len() && j < y.len() && x[i] == y[j] {
            open = false;
            i += 1;
            j += 1;
            continue;
        }
        if !open {
            out.push(Hunk { old_at: head + i, new_at: head + j, before: vec![], after: vec![] });
            open = true;
        }
        let h = out.last_mut().expect("a hunk is open");
        if j < y.len() && (i == x.len() || l[i][j + 1] >= l[i + 1][j]) {
            h.after.push(y[j].to_string());
            j += 1;
        } else {
            h.before.push(x[i].to_string());
            i += 1;
        }
    }
    // A pure insertion or deletion inside repeated lines could sit at several places; like git,
    // slide it down as far as the text allows, so the headers match `git diff`.
    for k in 0..out.len() {
        let next = out.get(k + 1).map(|h| (h.old_at, h.new_at));
        let h = &mut out[k];
        loop {
            let (o, n) = (h.old_at, h.new_at);
            let free = next.is_none_or(|(no, nn)| o + h.before.len() < no && n + h.after.len() < nn);
            if h.before.is_empty() && !h.after.is_empty() {
                let end = n + h.after.len();
                if !free || end >= head + y.len() + tail || b[end] != h.after[0] {
                    break;
                }
                h.after.rotate_left(1);
            } else if h.after.is_empty() && !h.before.is_empty() {
                let end = o + h.before.len();
                if !free || end >= head + x.len() + tail || a[end] != h.before[0] {
                    break;
                }
                h.before.rotate_left(1);
            } else {
                break;
            }
            h.old_at += 1;
            h.new_at += 1;
        }
    }
    out.into_iter().map(|h| Hunk::new(h.old_at, h.new_at, h.before, h.after)).collect()
}

/// Cells of the longest-common-subsequence table a preview builds at most (4 bytes each).
const MAX_CELLS: usize = 4_000_000;

impl Hunk {
    /// A hunk after `old` lines of the old text and `new` lines of the new one. As in a unified
    /// diff (C159), a side with lines starts at its first line and an empty side at the line
    /// before the change (0 at the very start).
    fn new(old: usize, new: usize, before: Vec<String>, after: Vec<String>) -> Hunk {
        let at = |n: usize, lines: &Vec<String>| if lines.is_empty() { n } else { n + 1 };
        Hunk { old_at: at(old, &before), new_at: at(new, &after), before, after }
    }
}
