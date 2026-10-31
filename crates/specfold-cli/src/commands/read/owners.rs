//! `spec owners`.

use crate::cli::args::{global::Global, read::OwnersArgs};
use crate::output::Output;
use crate::session;
use anyhow::Result;
use serde_json::json;

pub fn run(g: &Global, a: &OwnersArgs) -> Result<Output> {
    let s = session::open(g)?;
    let c = &s.corpus;
    let mut o = Output::new("owners");
    for ow in &c.owners.list {
        let n = c.docs.iter().filter(|d| d.id.owner == ow.id).count();
        if n == 0 && !a.empty && !ow.path.is_empty() {
            continue;
        }
        let stores_in = c.owners.storage_owner(ow);
        let mut text = format!("{:<32} {:>5}  {}", ow.id, n, if ow.path.is_empty() { "." } else { &ow.path });
        if ow.public {
            text.push_str(&format!("  public, documents in {}", stores_in.id));
        }
        if ow.retired {
            text.push_str("  retired");
        }
        o = o.item(json!({"id": ow.id, "path": ow.path, "docs": n, "public": ow.public, "stores_in": stores_in.id, "retired": ow.retired}), text);
    }
    let pick = o
        .items
        .iter()
        .find(|i| i.json["docs"].as_u64() > Some(0))
        .and_then(|i| i.json["id"].as_str().map(str::to_string));
    Ok(match pick {
        Some(id) => o.next(format!("spec find {}", specfold_core::shell::opt("--owner", &id.to_string()))),
        None => o,
    })
}
