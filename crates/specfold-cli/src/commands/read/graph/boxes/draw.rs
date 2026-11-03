//! The drawing as a whole: which nodes and edges it shows, and the refusal when it would not fit.

use super::canvas::{Canvas, draw_box};
use super::layout::{Layout, layers_of, order, slots_of};

/// The largest graph drawn as boxes; past this the tree reads better.
pub const MAX_NODES: usize = 60;

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
    let longest =
        (0..n).filter(|i| keep[*i]).flat_map(|i| labels[i].lines().map(|l| l.chars().count())).max().unwrap_or(0);
    let layout = Layout::fit(&slots, &hops, &row, &cols, layers, columns, longest)?;
    let mut canvas = Canvas::new(&layout, ascii);
    for (s, (l, node)) in slots.iter().enumerate() {
        let (x, y) = (layout.x0[*l], layout.mid(s));
        match node {
            Some(i) => draw_box(&mut canvas.ch, &labels[*i], x, y, layout.w, ascii),
            None => canvas.line(y, x, x + layout.w - 1),
        }
    }
    for hop in &hops {
        canvas.hop(&layout, &slots, hop);
    }
    let mut lines = canvas.lines();
    for (f, t, k) in &back {
        lines.push(format!("also: {} {k} {} (a cycle)", first_line(&labels[*f]), first_line(&labels[*t])));
    }
    Ok(Drawing { lines, width: layout.width })
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
