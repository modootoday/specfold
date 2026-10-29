//! Rendering an output as text, JSON or JSON lines, with paging applied.

use super::envelope::{Code, Output};
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Text,
    Json,
    Jsonl,
}

pub struct Page {
    pub limit: usize,
    pub offset: usize,
    /// Byte budget for the shown items; None means no budget.
    pub max_bytes: Option<usize>,
}

/// The items on this page and how many follow. A byte budget keeps whole items while they fit
/// (never fewer than one, never cutting an item), so a page of long items gets shorter instead
/// of larger; `cut` says the budget, not --limit, ended the page.
fn window<T: Clone>(items: &[T], page: &Page, paged: bool, size: impl Fn(&T) -> usize) -> (Vec<T>, usize, bool) {
    if !paged {
        return (items.to_vec(), 0, false);
    }
    let start = page.offset.min(items.len());
    let mut end = (start + page.limit).min(items.len());
    let mut cut = false;
    if let Some(budget) = page.max_bytes {
        let mut used = 0;
        for (n, item) in items[start..end].iter().enumerate() {
            used += size(item) + 1;
            if used > budget && n > 0 {
                end = start + n;
                cut = true;
                break;
            }
        }
    }
    (items[start..end].to_vec(), items.len() - end, cut)
}

pub fn render(o: &Output, fmt: Format, page: &Page) -> String {
    let (shown, more, cut) = window(&o.items, page, o.paged, |i| match fmt {
        Format::Text => i.text.len(),
        _ => i.json.to_string().len(),
    });
    match fmt {
        Format::Text => {
            let mut lines: Vec<String> = o.head.clone();
            lines.extend(shown.iter().map(|i| i.text.clone()));
            if more > 0 {
                let wider = if cut { "a larger --max-bytes".to_string() } else { format!("--limit {}", o.items.len()) };
                lines.push(format!("+{more} more not shown: --offset {} (or {wider})", page.offset + shown.len()));
            }
            lines.extend(o.foot.iter().cloned());
            if let Some(n) = &o.next {
                lines.push(format!("next: {n}"));
            }
            let mut s = lines.join("\n");
            if !s.is_empty() {
                s.push('\n');
            }
            s
        }
        Format::Json => {
            let mut obj = serde_json::Map::new();
            obj.insert("schema".into(), json!(o.schema));
            obj.insert("ok".into(), json!(o.code == Code::Ok));
            obj.insert("exit".into(), json!(o.code as u8));
            obj.extend(o.summary.clone());
            obj.insert("items".into(), Value::Array(shown.iter().map(|i| i.json.clone()).collect()));
            obj.insert("total".into(), json!(o.items.len()));
            if o.paged {
                obj.insert("offset".into(), json!(page.offset));
                obj.insert("more".into(), json!(more));
                if cut {
                    obj.insert("cut_by".into(), json!("max_bytes"));
                }
            }
            obj.insert("next".into(), json!(o.next));
            format!("{}\n", Value::Object(obj))
        }
        Format::Jsonl => {
            let mut s = String::new();
            for i in &shown {
                s.push_str(&i.json.to_string());
                s.push('\n');
            }
            let mut tail = serde_json::Map::new();
            tail.insert("schema".into(), json!(o.schema));
            tail.insert("exit".into(), json!(o.code as u8));
            tail.insert("total".into(), json!(o.items.len()));
            tail.insert("more".into(), json!(more));
            if cut {
                tail.insert("cut_by".into(), json!("max_bytes"));
            }
            tail.insert("next".into(), json!(o.next));
            s.push_str(&Value::Object(tail).to_string());
            s.push('\n');
            s
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_and_suggests() {
        let mut o = Output::new("find").next("spec show a");
        for i in 0..5 {
            o = o.item(json!({"i": i}), format!("item {i}"));
        }
        let t = render(&o, Format::Text, &Page { limit: 2, offset: 1, max_bytes: None });
        assert_eq!(t, "item 1\nitem 2\n+2 more not shown: --offset 3 (or --limit 5)\nnext: spec show a\n");
        let j: Value =
            serde_json::from_str(&render(&o, Format::Json, &Page { limit: 2, offset: 0, max_bytes: None })).unwrap();
        assert_eq!(j["schema"], "spec.find.v1");
        assert_eq!(j["total"], 5);
        assert_eq!(j["more"], 3);
        assert_eq!(j["next"], "spec show a");
    }
}
