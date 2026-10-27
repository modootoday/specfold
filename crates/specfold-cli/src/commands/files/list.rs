//! `spec files`.

use crate::cli::args::{global::Global, read::RefArgs};
use crate::output::Output;
use crate::session;
use anyhow::Result;
use serde_json::json;

pub fn run(g: &Global, a: &RefArgs) -> Result<Output> {
    let s = session::open(g)?;
    let idx = session::resolve(&s, &a.doc)?;
    let d = &s.corpus.docs[idx];
    let base = d.base_dir();
    let mut o = Output::new("files").field("id", d.id.to_string()).field("base", base.clone());
    for f in d.files() {
        let present = s.corpus.cfg.root.join(&base).join(&f.path).is_file();
        let note = if present { "" } else { "  (not on disk)" };
        o = o.item(
            json!({"path": f.path, "sha256": f.sha256, "bytes": f.bytes, "store": f.store, "on_disk": present}),
            format!("{:<5} {:>10}  {}{note}", f.store, f.bytes, f.path),
        );
    }
    Ok(o.next(format!("spec fetch {}", d.id)))
}
